#requires -Version 7
<#
.SYNOPSIS
  Receipt probe — verify an executor CLI actually RECEIVED and acted on a dispatched spec,
  not merely that a process started or exited 0.

.DESCRIPTION
  Generates a unique token, builds a tiny spec instructing the target to write that token
  into a receipt file, feeds the spec via stdin, then asserts the receipt file contains the
  token. A matching token is the only authoritative proof of real receipt (file-content
  verification, the same principle the pipeline uses for stage write-back).

  Observability across the five providers:
    - Headless-capable (claude, opencode, agy): can receive — probed for real.
    - Non-headless (codex, copilot): cannot receive a piped spec — reported 'non-dispatchable'.
      That verdict IS the observation for them.
    - echo: built-in mock that validates this harness with zero external dependency.

  Exit: 0 receipt confirmed · 1 ran but no receipt · 2 unavailable / timeout / non-dispatchable.
#>
param(
    [Parameter(Mandatory)]
    [ValidateSet('echo', 'claude', 'opencode', 'agy', 'codex', 'copilot')]
    [string]$Executor,
    [int]$TimeoutSeconds = 60
)

$ErrorActionPreference = 'Stop'

# Providers with no headless / non-interactive mode — structurally cannot receive a piped spec.
$nonHeadless = @('codex', 'copilot')

# Headless invocation templates. The spec is ALWAYS fed via stdin (never as an arg).
# Vendor flags are SPIKE-PENDING (plan headless-cli-pipeline Step 2 / T-002) — confirm before trusting.
$templates = @{
    claude   = @{ Exe = 'claude';   Args = @('-p', '--dangerously-skip-permissions') }
    opencode = @{ Exe = 'opencode'; Args = @('run', '--dangerously-skip-permissions') }
    agy      = @{ Exe = 'agy';      Args = @('-p', '--dangerously-skip-permissions') }
}

function Write-Verdict([string]$Status, [string]$Detail) {
    Write-Host ("[{0}] receipt={1} — {2}" -f $Executor, $Status, $Detail)
}

# Non-headless providers: the observation is that they cannot receive.
if ($Executor -in $nonHeadless) {
    Write-Verdict 'non-dispatchable' "$Executor has no headless mode; a piped spec cannot reach it."
    exit 2
}

$token = [guid]::NewGuid().ToString('N')
$workDir = Join-Path ([System.IO.Path]::GetTempPath()) "gal-receipt-$token"
New-Item -ItemType Directory -Path $workDir -Force | Out-Null
$receipt = Join-Path $workDir 'receipt.txt'
$spec = @"
# Receipt Probe Spec
Write exactly this token to the file '$receipt', and nothing else: $token
Do not run any other command. Do not git commit or push.
"@

try {
    if ($Executor -eq 'echo') {
        # Mock a compliant executor that obeys the spec.
        Set-Content -LiteralPath $receipt -Value $token -NoNewline
    }
    else {
        $tpl = $templates[$Executor]
        if (-not (Get-Command $tpl.Exe -ErrorAction SilentlyContinue)) {
            Write-Verdict 'unavailable' "$($tpl.Exe) not found on PATH."
            exit 2
        }
        # Feed spec via stdin, bounded by timeout. Production dispatch (Invoke-Executor.ps1)
        # reclaims the full process tree via taskkill /T; this probe uses a job for simplicity.
        $job = Start-Job -ScriptBlock {
            param($exe, $jobArgs, $stdin, $dir)
            Set-Location $dir
            $stdin | & $exe @jobArgs
        } -ArgumentList $tpl.Exe, $tpl.Args, $spec, $workDir

        if (-not (Wait-Job $job -Timeout $TimeoutSeconds)) {
            Stop-Job $job
            Remove-Job $job -Force
            Write-Verdict 'timeout' "No receipt within ${TimeoutSeconds}s."
            exit 2
        }
        Receive-Job $job | Out-Null
        Remove-Job $job -Force
    }

    # The only authoritative check: did the token land in the receipt file?
    if ((Test-Path $receipt) -and ((Get-Content -Raw $receipt).Trim() -eq $token)) {
        Write-Verdict 'ok' 'token confirmed — executor genuinely received and acted on the spec.'
        exit 0
    }

    Write-Verdict 'no-receipt' 'process finished but token not found — spec was not acted on.'
    exit 1
}
finally {
    Remove-Item -LiteralPath $workDir -Recurse -Force -ErrorAction SilentlyContinue
}
