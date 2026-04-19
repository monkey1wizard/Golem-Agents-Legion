<#
.SYNOPSIS
    Run a GAL task on this worker node using Gemini CLI in headless mode.

.DESCRIPTION
    Intended to run on the remote worker machine (invoked by Invoke-GalRemoteTask.ps1).
    Executes the task spec via Gemini CLI non-interactively, captures output,
    generates a result patch, and writes all output files to the output directory.

    This script is NOT intended to be run manually in normal use. It is invoked
    remotely by the control plane. For debugging, it can be run locally with
    explicit parameters.

.PARAMETER TaskId
    The task ID assigned by Invoke-GalRemoteTask.

.PARAMETER WorktreePath
    Absolute path to the isolated git worktree for this task.

.PARAMETER TaskSpec
    Absolute path to the task spec Markdown file.

.PARAMETER OutputDir
    Absolute path to the directory where output files will be written.

.EXAMPLE
    .\Start-GalWorker.ps1 `
        -TaskId "20260101-abc123" `
        -WorktreePath "C:\Code\MyRepo-worker-20260101-abc123" `
        -TaskSpec "C:\Windows\Temp\gal-worker\20260101-abc123\task.md" `
        -OutputDir "C:\Windows\Temp\gal-worker\20260101-abc123"
#>

param(
    [Parameter(Mandatory)]
    [string]$TaskId,

    [Parameter(Mandatory)]
    [string]$WorktreePath,

    [Parameter(Mandatory)]
    [string]$TaskSpec,

    [Parameter(Mandatory)]
    [string]$OutputDir,

    [int]$TimeoutMinutes = 30
)

$ErrorActionPreference = "Stop"

$statusPath  = Join-Path $OutputDir "status.json"
$summaryPath = Join-Path $OutputDir "summary.md"
$logPath     = Join-Path $OutputDir "worker.log"
$patchPath   = Join-Path $OutputDir "result.patch"

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

function Write-StatusJson([string]$Status, [int]$ExitCode = 0, [string]$ErrorMessage = $null) {
    $payload = [ordered]@{
        taskId       = $TaskId
        status       = $Status
        exitCode     = $ExitCode
        startedAt    = $script:startedAt
        finishedAt   = (Get-Date -Format "o")
        engine       = "gemini-cli"
        worktreePath = $WorktreePath
        errorMessage = $ErrorMessage
    }
    $payload | ConvertTo-Json | Set-Content -Path $statusPath -Encoding UTF8
}

# ── Validate prerequisites ────────────────────────────────────────────────────
if (-not (Test-Path $WorktreePath)) {
    Write-StatusJson "failed" -ExitCode 1 -ErrorMessage "Worktree not found: $WorktreePath"
    exit 1
}
if (-not (Test-Path $TaskSpec)) {
    Write-StatusJson "failed" -ExitCode 42 -ErrorMessage "Task spec not found: $TaskSpec"
    exit 1
}
if (-not (Get-Command gemini -ErrorAction SilentlyContinue)) {
    Write-StatusJson "failed" -ExitCode 1 -ErrorMessage "gemini command not found on PATH"
    exit 1
}

# ── Write initial running status ──────────────────────────────────────────────
$script:startedAt = (Get-Date -Format "o")
Write-StatusJson "running"

# ── Read task spec as prompt ──────────────────────────────────────────────────
$taskPrompt = Get-Content -Path $TaskSpec -Raw

# ── Run Gemini CLI non-interactively with timeout ─────────────────────────────
# Runs inside a Start-Job so the process can be killed if it exceeds TimeoutMinutes.
# stdin is closed ($null |) to prevent interactive consent prompts.
# --yolo skips tool-use approval prompts.
# Environment variables (e.g. GOOGLE_GEMINI_API_KEY) are inherited by child processes.
$geminiExitCode = 0

$geminiJob = Start-Job -ScriptBlock {
    param($wt, $prompt, $log)
    Set-Location $wt
    $null | gemini --yolo -p $prompt --output-format stream-json *> $log
    $LASTEXITCODE
} -ArgumentList $WorktreePath, $taskPrompt, $logPath

$timeoutSeconds = $TimeoutMinutes * 60
$finished = Wait-Job $geminiJob -Timeout $timeoutSeconds

if (-not $finished) {
    Stop-Job $geminiJob
    Remove-Job $geminiJob -Force
    $geminiExitCode = -1
    "`nTIMEOUT: Gemini CLI exceeded ${TimeoutMinutes}m — task killed" | Add-Content -Path $logPath
    Write-StatusJson "timeout" -ExitCode -1 -ErrorMessage "Task exceeded ${TimeoutMinutes}m timeout — split the task or increase -TimeoutMinutes"
    exit 1
}

try {
    $jobOutput = Receive-Job $geminiJob -ErrorVariable jobErrors 2>$null
    $geminiExitCode = if ($null -ne $jobOutput) { [int]($jobOutput | Select-Object -Last 1) } else { 0 }
    if ($jobErrors) {
        $jobErrors | ForEach-Object { "JOB ERROR: $_" | Add-Content -Path $logPath }
    }
}
catch {
    $geminiExitCode = 1
    "EXCEPTION receiving job result: $_" | Add-Content -Path $logPath
}
finally {
    Remove-Job $geminiJob -Force -ErrorAction SilentlyContinue
}

# ── Generate result patch ─────────────────────────────────────────────────────
try {
    $patchContent = git -C $WorktreePath diff HEAD 2>&1
    if ($patchContent) {
        $patchContent | Set-Content -Path $patchPath -Encoding UTF8
    }
    else {
        "" | Set-Content -Path $patchPath -Encoding UTF8
    }
}
catch {
    "Failed to generate patch: $_" | Set-Content -Path $patchPath -Encoding UTF8
}

# ── Extract summary from log ──────────────────────────────────────────────────
# Tries three strategies to extract human-readable text from stream-json output:
#   1. Top-level .text field
#   2. .content.parts[].text (Gemini content block)
#   3. .candidates[].content.parts[].text
# Falls back to the last 20 non-empty log lines if no JSON text is found.
$summaryContent = ""
try {
    $logLines = Get-Content -Path $logPath -ErrorAction SilentlyContinue
    if ($logLines) {
        $textParts = [System.Collections.Generic.List[string]]::new()

        foreach ($line in $logLines) {
            if ($line -notmatch '^\s*\{') { continue }
            try {
                $obj = $line | ConvertFrom-Json -ErrorAction SilentlyContinue
                if (-not $obj) { continue }

                # Strategy 1: top-level .text field
                if ($obj.text) { $textParts.Add([string]$obj.text); continue }

                # Strategy 2: .content.parts[].text
                if ($obj.content -and $obj.content.parts) {
                    foreach ($part in $obj.content.parts) {
                        if ($part.text) { $textParts.Add([string]$part.text) }
                    }
                    continue
                }

                # Strategy 3: .candidates[].content.parts[].text
                if ($obj.candidates) {
                    foreach ($cand in $obj.candidates) {
                        if ($cand.content -and $cand.content.parts) {
                            foreach ($part in $cand.content.parts) {
                                if ($part.text) { $textParts.Add([string]$part.text) }
                            }
                        }
                    }
                }
            }
            catch { }
        }

        if ($textParts.Count -gt 0) {
            $summaryContent = $textParts -join ""
        }
        else {
            $lastLines = ($logLines | Where-Object { $_ -match '\S' } | Select-Object -Last 20) -join "`n"
            $summaryContent = "(Could not parse stream-json — raw log tail)`n`n$lastLines"
        }
    }
}
catch {
    $summaryContent = "(Could not extract summary from worker.log — check raw log)"
}

$summaryLines = @(
    "# Task Summary: $TaskId",
    "",
    "**Status**: $(if ($geminiExitCode -eq 0) { 'success' } else { "failed (exit $geminiExitCode)" })",
    "",
    "## Output",
    "",
    $summaryContent
)
$summaryLines | Set-Content -Path $summaryPath -Encoding UTF8

# ── Lock worktree (preserve until retrieval) ──────────────────────────────────
try {
    git -C $WorktreePath worktree lock $WorktreePath --reason "gal-worker-$TaskId" 2>&1 | Out-Null
}
catch {
    # Non-fatal — lock failure won't block retrieval
}

# ── Write final status ────────────────────────────────────────────────────────
# Map known Gemini CLI exit codes
$exitCodeMessages = @{
    41 = "Gemini CLI auth failed — re-auth required on worker before next task"
    42 = "Gemini CLI input error — task spec may be malformed"
    44 = "Gemini CLI sandbox error — check worker sandbox configuration"
    52 = "Gemini CLI config error — check GEMINI.md or settings.json on worker"
    53 = "Gemini CLI turn limit reached — split task into smaller pieces"
}

if ($geminiExitCode -eq 0) {
    Write-StatusJson "success"
    Write-Host "Worker finished: $TaskId — success"
}
else {
    $msg = $exitCodeMessages[$geminiExitCode]
    if (-not $msg) { $msg = "Gemini CLI exited with code $geminiExitCode — check worker.log" }
    Write-StatusJson "failed" -ExitCode $geminiExitCode -ErrorMessage $msg
    Write-Host "Worker finished: $TaskId — FAILED (exit $geminiExitCode): $msg"
}
