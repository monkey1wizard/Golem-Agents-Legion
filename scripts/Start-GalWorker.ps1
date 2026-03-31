<#
.SYNOPSIS
    Run a GAL task on this worker node using Gemini CLI in headless mode.

.DESCRIPTION
    Intended to run on the remote worker machine (invoked by Invoke-GalRemoteTask.ps1).
    Executes the task spec via Gemini CLI non-interactively, captures output,
    generates a result patch, and writes all artifacts to the output directory.

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
    Absolute path to the directory where artifacts will be written.

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
    [string]$OutputDir
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

# ── Run Gemini CLI non-interactively ──────────────────────────────────────────
# - stdin is closed ($null |) to prevent any interactive consent prompts
# - --yolo skips tool-use approval prompts
# - output is captured to worker.log; summary is extracted from the final response
Set-Location $WorktreePath

$geminiExitCode = 0

try {
    $null | gemini --yolo -p $taskPrompt --output-format stream-json *> $logPath
    $geminiExitCode = $LASTEXITCODE
}
catch {
    $geminiExitCode = 1
    "EXCEPTION: $_" | Add-Content -Path $logPath
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
# Parse stream-json log: look for the last content block from the model
$summaryContent = ""
try {
    $logLines = Get-Content -Path $logPath -ErrorAction SilentlyContinue
    # Collect all text parts from stream-json output
    $textParts = $logLines | Where-Object { $_ -match '"text"\s*:' } | ForEach-Object {
        try {
            ($_ | ConvertFrom-Json -ErrorAction SilentlyContinue).text
        } catch { $null }
    } | Where-Object { $_ -ne $null }

    if ($textParts) {
        $summaryContent = $textParts -join ""
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
