param(
    [Parameter(Mandatory)]
    [string]$RunId,

    [Parameter(Mandatory)]
    [string]$WorktreePath,

    [Parameter(Mandatory)]
    [string]$PlanSnapshot,

    [Parameter(Mandatory)]
    [string]$StateSnapshot,

    [Parameter(Mandatory)]
    [string]$OutputDir,

    [int]$TimeoutMinutes = 30
)

$ErrorActionPreference = 'Stop'

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

$statusPath = Join-Path $OutputDir 'pipeline-status.json'
$eventsPath = Join-Path $OutputDir 'pipeline-events.jsonl'
$tasksDir = Join-Path $OutputDir 'tasks'
New-Item -ItemType Directory -Force -Path $tasksDir | Out-Null

$startedAt = (Get-Date).ToUniversalTime().ToString('o')

function New-EventJson {
    param(
        [Parameter(Mandatory)][string]$Type,
        [Parameter(Mandatory)][object]$Details
    )

    return [ordered]@{
        timestamp = (Get-Date).ToUniversalTime().ToString('o')
        type = $Type
        details = $Details
    } | ConvertTo-Json -Depth 10 -Compress
}

function Add-Event {
    param(
        [Parameter(Mandatory)][string]$Type,
        [Parameter(Mandatory)][object]$Details
    )

    Add-Content -Path $eventsPath -Value (New-EventJson -Type $Type -Details $Details)
}

function Write-PipelineStatus {
    param(
        [Parameter(Mandatory)][string]$Status,
        [Parameter(Mandatory)][string]$Phase,
        [Parameter(Mandatory)][object[]]$Tasks,
        [Parameter(Mandatory)][string]$Message
    )

    $payload = [ordered]@{
        runId = $RunId
        status = $Status
        phase = $Phase
        startedAt = $startedAt
        updatedAt = (Get-Date).ToUniversalTime().ToString('o')
        timeoutMinutes = $TimeoutMinutes
        worktreePath = $WorktreePath
        planSnapshotPath = $PlanSnapshot
        stateSnapshotPath = $StateSnapshot
        message = $Message
        tasks = $Tasks
    }

    $payload | ConvertTo-Json -Depth 10 | Set-Content -Path $statusPath -Encoding UTF8
}

if (-not (Test-Path $WorktreePath)) {
    throw "Missing worktree: $WorktreePath"
}

if (-not (Test-Path $PlanSnapshot)) {
    throw "Missing plan snapshot: $PlanSnapshot"
}

if (-not (Test-Path $StateSnapshot)) {
    throw "Missing state snapshot: $StateSnapshot"
}

$taskLines = Get-Content -Path $PlanSnapshot | Where-Object { $_ -match '^- \[[ x]\] T-[0-9]{3} ' }
$tasks = [System.Collections.Generic.List[object]]::new()

foreach ($taskLine in $taskLines) {
    $taskId = [regex]::Match($taskLine, 'T-[0-9]{3}').Value
    if ([string]::IsNullOrWhiteSpace($taskId)) {
        continue
    }

    $taskDir = Join-Path $tasksDir ($taskId.ToLowerInvariant())
    New-Item -ItemType Directory -Force -Path $taskDir | Out-Null

    $tasks.Add([ordered]@{
        taskId = $taskId
        planLine = $taskLine
        complete = $taskLine.StartsWith('- [x] ')
        status = 'queued'
        taskDir = $taskDir
        phases = @()
    })
}

Add-Event -Type 'runner-started' -Details ([ordered]@{ message = 'persistent xmachine pipeline bootstrap started'; platform = 'windows' })
Add-Event -Type 'plan-loaded' -Details ([ordered]@{ planSnapshot = $PlanSnapshot; stateSnapshot = $StateSnapshot; taskCount = $tasks.Count })
Write-PipelineStatus -Status 'initialized' -Phase 'bootstrap-complete' -Tasks $tasks.ToArray() -Message 'Persistent xmachine pipeline runner initialized. Task execution loop is the next implementation slice.'
Add-Event -Type 'bootstrap-complete' -Details ([ordered]@{ taskCount = $tasks.Count; message = 'pipeline status scaffold created' })
