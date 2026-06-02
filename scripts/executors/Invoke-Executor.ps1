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

    [int]$TimeoutMinutes = 10,

    # Optional: task scope (e.g. "T-007") and phase for the durable run-record filename.
    # Auto-extracted from TaskSpecPath when omitted (e.g. .dev/task-specs/T-007-implement.md).
    [string]$TaskScope,
    [string]$Phase,

    # Maximum number of log files to retain in .dev/executor-logs/ across all executors.
    [int]$LogRetentionCount = 50
)

$ErrorActionPreference = 'Stop'
$scriptRoot = $PSScriptRoot
$repoRoot   = Split-Path (Split-Path $scriptRoot -Parent) -Parent

# ---------------------------------------------------------------------------
# T-013: Durable run-record helpers
# ---------------------------------------------------------------------------

# Auto-detect task scope and phase from spec path if not provided
if ((-not $TaskScope -or -not $Phase) -and $TaskSpecPath) {
    $specBasename = [System.IO.Path]::GetFileNameWithoutExtension($TaskSpecPath)
    if ($specBasename -match '^(T-\d+)-(.+)$') {
        if (-not $TaskScope) { $TaskScope = $Matches[1] }
        if (-not $Phase)     { $Phase     = $Matches[2] }
    }
}
$TaskScope = if ($TaskScope) { $TaskScope } else { 'unknown' }
$Phase     = if ($Phase)     { $Phase }     else { 'unknown' }

function Get-GitInfo {
    try {
        $branch = (git -C $repoRoot rev-parse --abbrev-ref HEAD 2>$null).Trim()
        $head   = (git -C $repoRoot rev-parse --short HEAD 2>$null).Trim()
        return "$branch / $head"
    } catch { return 'unknown' }
}

function Write-ExecutorLog {
    param(
        [string]$LogPath,
        [string]$TerminalState,
        [string]$StdOut,
        [string]$StdErr,
        [datetime]$StartTime,
        [int]$ExitCode
    )

    $endTime  = Get-Date
    $duration = ($endTime - $StartTime).TotalSeconds
    $gitInfo  = Get-GitInfo

    $header = @"
=== Executor Run Record ===
Executor:      $Executor
Task:          $TaskScope
Phase:         $Phase
Spec:          $TaskSpecPath
Git:           $gitInfo
Start:         $($StartTime.ToString('yyyy-MM-dd HH:mm:ss'))
End:           $($endTime.ToString('yyyy-MM-dd HH:mm:ss'))
Duration:      ${duration}s
Exit Code:     $ExitCode
Terminal State: $TerminalState
===========================

--- STDOUT ---
$StdOut
--- STDERR ---
$StdErr
--- END ---
"@
    [System.IO.File]::WriteAllText($LogPath, $header, [System.Text.UTF8Encoding]::new($false))
}

function Enforce-LogRetention {
    $logDir = Join-Path $repoRoot '.dev\executor-logs'
    if (-not (Test-Path $logDir)) { return }
    $logs = Get-ChildItem -LiteralPath $logDir -Filter '*.log' |
        Sort-Object LastWriteTime -Descending
    if ($logs.Count -gt $LogRetentionCount) {
        $logs | Select-Object -Skip $LogRetentionCount | ForEach-Object {
            Remove-Item -LiteralPath $_.FullName -Force -ErrorAction SilentlyContinue
        }
    }
}

function New-LogPath {
    $ts      = (Get-Date).ToString('yyyyMMdd-HHmmss')
    $logDir  = Join-Path $repoRoot '.dev\executor-logs'
    if (-not (Test-Path $logDir)) {
        New-Item -ItemType Directory -Path $logDir -Force | Out-Null
    }
    return Join-Path $logDir "$ts-$TaskScope-$Phase-$Executor.log"
}

$runStartTime = Get-Date

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
    $lp = New-LogPath
    Write-ExecutorLog -LogPath $lp -TerminalState 'completed' -StdOut $specContent -StdErr '' -StartTime $runStartTime -ExitCode 0
    Enforce-LogRetention
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
        $lp = New-LogPath
        Write-ExecutorLog -LogPath $lp -TerminalState 'timeout' -StdOut '' -StdErr '' -StartTime $runStartTime -ExitCode 2
        Enforce-LogRetention
        exit 2
    }

    $null = [System.Threading.Tasks.Task]::WhenAll($stdoutTask, $stderrTask)
    $lp = New-LogPath
    Write-ExecutorLog -LogPath $lp -TerminalState 'completed' -StdOut $stdoutTask.Result -StdErr $stderrTask.Result -StartTime $runStartTime -ExitCode 0
    Enforce-LogRetention
    exit 0
}

# ---------------------------------------------------------------------------
# Real executor — locate the adapter script
# ---------------------------------------------------------------------------
$adapterPath = Join-Path $scriptRoot "$Executor.ps1"
if (-not (Test-Path $adapterPath)) {
    [Console]::Error.WriteLine("Invoke-Executor: executor '$Executor' not found — no adapter at '$adapterPath'")
    $lp = New-LogPath
    Write-ExecutorLog -LogPath $lp -TerminalState 'unavailable' -StdOut '' -StdErr "No adapter at '$adapterPath'" -StartTime $runStartTime -ExitCode 2
    Enforce-LogRetention
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
    $lp = New-LogPath
    Write-ExecutorLog -LogPath $lp -TerminalState 'timeout' -StdOut '' -StdErr "Timed out after $TimeoutMinutes min" -StartTime $runStartTime -ExitCode 2
    Enforce-LogRetention
    exit 2
}

$exitCode = $proc.ExitCode
$terminalState = if ($exitCode -eq 0) { 'completed' }
                 elseif ($exitCode -eq 1) { 'no-receipt' }
                 else { 'disconnected-partial' }

$lp = New-LogPath
Write-ExecutorLog -LogPath $lp -TerminalState $terminalState -StdOut $stdout -StdErr $stderr -StartTime $runStartTime -ExitCode $exitCode
Enforce-LogRetention

exit $exitCode
