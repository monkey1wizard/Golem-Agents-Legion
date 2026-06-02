#Requires -Version 7
<#
.SYNOPSIS
  Unified local executor entry point for headless CLI dispatch.

.DESCRIPTION
  Dispatches a task spec to a named executor CLI (claude, opencode, agy) or a
  built-in mock (echo, sleep).  Spec is always delivered via stdin — never as a
  command-line argument — to avoid quoting / length / newline issues.

  Exit codes:
    0  Task completed successfully (adapter exit 0).
    1  Task ran but failed (adapter exit 1).
    2  CLI unavailable, adapter not found, or timeout.

  Built-in mocks:
    echo   Reads TaskSpecPath and prints it; exits 0. Zero external dependency.
    sleep  Spawns a long-running child process; exits 2 when timeout fires.
           Use this to test the timeout + process-tree-kill path (TP-003).

  Adapter lookup: scripts/executors/<Executor>.ps1
  If the adapter is absent the executor is treated as unavailable → exit 2.

  Timeout:  taskkill /T /F kills the entire process tree (claude/node/opencode
            spawn children that a plain Stop-Process would leave as orphans).

  Note: durable run-record logging is added in T-013.
#>
param(
    [Parameter(Mandatory)]
    [string]$Executor,

    [string]$TaskSpecPath,

    [string]$WorkDir = $PWD.Path,

    [int]$TimeoutMinutes = 10
)

$ErrorActionPreference = 'Stop'
$scriptRoot = $PSScriptRoot

# ---------------------------------------------------------------------------
# Helper: kill the whole process tree and all descendants on Windows
# ---------------------------------------------------------------------------
function Stop-ProcessTree {
    param([int]$TargetPid)
    & taskkill /T /F /PID $TargetPid 2>$null | Out-Null
}

# ---------------------------------------------------------------------------
# Helper: spawn a child process with stdin/stdout/stderr redirected
# ---------------------------------------------------------------------------
function Start-AdapterProcess {
    param([string]$AdapterFile, [string]$WorkDirectory)

    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName               = 'pwsh'
    $psi.Arguments              = "-NonInteractive -File `"$AdapterFile`" -WorkDir `"$WorkDirectory`""
    $psi.RedirectStandardInput  = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError  = $true
    $psi.UseShellExecute        = $false
    $psi.WorkingDirectory       = $WorkDirectory

    return [System.Diagnostics.Process]::Start($psi)
}

# ---------------------------------------------------------------------------
# Helper: wait for a process with timeout; kill process tree on timeout
# ---------------------------------------------------------------------------
function Wait-ProcessWithTimeout {
    param(
        [System.Diagnostics.Process]$Process,
        [int]$TimeoutMs
    )

    # Drain async to prevent deadlock when stdout/stderr buffers fill
    $stdoutTask = $Process.StandardOutput.ReadToEndAsync()
    $stderrTask = $Process.StandardError.ReadToEndAsync()

    $completed = $Process.WaitForExit($TimeoutMs)

    if (-not $completed) {
        Stop-ProcessTree -TargetPid $Process.Id
        $null = [System.Threading.Tasks.Task]::WhenAll($stdoutTask, $stderrTask)
        return $false, '', ''
    }

    $null = [System.Threading.Tasks.Task]::WhenAll($stdoutTask, $stderrTask)
    return $true, $stdoutTask.Result, $stderrTask.Result
}

# ---------------------------------------------------------------------------
# Read spec content (safe even when TaskSpecPath is absent)
# ---------------------------------------------------------------------------
$specContent = ''
if ($TaskSpecPath -and (Test-Path $TaskSpecPath)) {
    $specContent = Get-Content $TaskSpecPath -Raw -Encoding UTF8
}

$timeoutMs = $TimeoutMinutes * 60 * 1000

# ---------------------------------------------------------------------------
# echo mock — zero external dependency; validates the harness itself
# ---------------------------------------------------------------------------
if ($Executor -eq 'echo') {
    Write-Output $specContent
    exit 0
}

# ---------------------------------------------------------------------------
# sleep mock — spawns a blocking child to exercise timeout + tree-kill (TP-003)
# ---------------------------------------------------------------------------
if ($Executor -eq 'sleep') {
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName               = 'pwsh'
    $psi.Arguments              = '-NonInteractive -Command "Start-Sleep 3600"'
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError  = $true
    $psi.UseShellExecute        = $false

    $sleepProc = [System.Diagnostics.Process]::Start($psi)
    $stdoutTask = $sleepProc.StandardOutput.ReadToEndAsync()
    $stderrTask = $sleepProc.StandardError.ReadToEndAsync()

    $completed = $sleepProc.WaitForExit($timeoutMs)
    if (-not $completed) {
        Stop-ProcessTree -TargetPid $sleepProc.Id
        $null = [System.Threading.Tasks.Task]::WhenAll($stdoutTask, $stderrTask)
        exit 2
    }

    $null = [System.Threading.Tasks.Task]::WhenAll($stdoutTask, $stderrTask)
    exit 0
}

# ---------------------------------------------------------------------------
# Real executor — locate the adapter script
# ---------------------------------------------------------------------------
$adapterPath = Join-Path $scriptRoot "$Executor.ps1"
if (-not (Test-Path $adapterPath)) {
    [Console]::Error.WriteLine("Invoke-Executor: executor '$Executor' not found — no adapter at '$adapterPath'")
    exit 2
}

# ---------------------------------------------------------------------------
# Spawn adapter, pipe spec via stdin, wait with timeout
# ---------------------------------------------------------------------------
$proc = Start-AdapterProcess -AdapterFile $adapterPath -WorkDirectory $WorkDir

$proc.StandardInput.Write($specContent)
$proc.StandardInput.Close()

$waited, $stdout, $stderr = Wait-ProcessWithTimeout -Process $proc -TimeoutMs $timeoutMs

if (-not $waited) {
    [Console]::Error.WriteLine("Invoke-Executor: executor '$Executor' timed out after $TimeoutMinutes min — process tree killed.")
    exit 2
}

$exitCode = $proc.ExitCode
exit $exitCode
