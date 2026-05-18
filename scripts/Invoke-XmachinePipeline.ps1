param(
    [Parameter(Mandatory)]
    [string]$WorkNode,

    [string]$PlanPath,

    [string]$WorkRepoPath,

    [string]$RemoteRuntimeRepoPath,

    [int]$TimeoutMinutes = 30,

    [switch]$KeepRemote
)

$ErrorActionPreference = "Stop"

function Get-RepoContextRoot {
    $current = (Get-Location).Path

    while ($true) {
        $statePath = Join-Path $current ".dev\state.md"
        if (Test-Path $statePath) {
            return $current
        }

        $parent = Split-Path $current -Parent
        if ([string]::IsNullOrWhiteSpace($parent) -or $parent -eq $current) {
            throw "Could not locate a target project root containing .dev\state.md from '$((Get-Location).Path)'."
        }

        $current = $parent
    }
}

function Get-StatePath([string]$RepoContextRoot) {
    return Join-Path $RepoContextRoot ".dev\state.md"
}

function Resolve-MarkdownCode([string]$Value) {
    if ($null -eq $Value) { return $null }
    $trimmed = $Value.Trim()
    if ($trimmed.Length -ge 2 -and $trimmed.StartsWith('`') -and $trimmed.EndsWith('`')) {
        return $trimmed.Substring(1, $trimmed.Length - 2)
    }

    return $trimmed
}

function Resolve-PlanPath {
    param(
        [string]$Value,
        [string]$RepoContextRoot
    )

    $candidate = Resolve-MarkdownCode $Value
    if ([string]::IsNullOrWhiteSpace($candidate)) {
        return $null
    }

    $candidate = $candidate -replace '/', '\\'
    $absolutePath = if ([System.IO.Path]::IsPathRooted($candidate)) {
        $candidate
    }
    else {
        Join-Path $RepoContextRoot $candidate
    }

    if ($absolutePath -match '\.prompt\.md$') {
        return $absolutePath
    }

    if ($absolutePath -match '\.md$') {
        $sourcePlanRoot = Join-Path $RepoContextRoot 'docs\plans'
        if ($absolutePath.StartsWith($sourcePlanRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
            $relativeSourcePlanPath = $absolutePath.Substring($sourcePlanRoot.Length).TrimStart('\\')
            $relativePromptPath = $relativeSourcePlanPath -replace '\.md$', '.prompt.md'
            $devPromptPath = Join-Path (Join-Path $RepoContextRoot '.dev\plans') $relativePromptPath
            if (Test-Path $devPromptPath) {
                return $devPromptPath
            }
        }

        $promptPath = $absolutePath -replace '\.md$', '.prompt.md'
        if (Test-Path $promptPath) {
            return $promptPath
        }
    }

    return $absolutePath
}

function Get-ActivePlanPath([string]$RepoContextRoot) {
    $statePath = Get-StatePath -RepoContextRoot $RepoContextRoot
    if (-not (Test-Path $statePath)) {
        return $null
    }

    $inActivePlans = $false
    $headerSeen = $false

    foreach ($line in Get-Content $statePath) {
        if ($line -match '^##\s+Active Plans\b') {
            $inActivePlans = $true
            continue
        }

        if ($inActivePlans -and $line -match '^##\s+') {
            break
        }

        if (-not $inActivePlans) { continue }
        if ($line -notmatch '^\|') { continue }
        if ($line -match '^\|\s*---') { continue }

        $cells = $line.Trim().Trim('|').Split('|') | ForEach-Object { $_.Trim() }
        if ($cells.Count -lt 2) { continue }

        if (-not $headerSeen) {
            if ($cells -contains 'File') {
                $headerSeen = $true
            }
            continue
        }

        foreach ($cell in $cells) {
            $resolved = Resolve-PlanPath -Value $cell -RepoContextRoot $RepoContextRoot
            if ($resolved -and $resolved -match '\.(prompt\.md|md)$') {
                return $resolved
            }
        }
    }

    return $null
}

function Get-RepoRoot {
    if ([string]::IsNullOrWhiteSpace($PSScriptRoot)) {
        throw "PSScriptRoot is not available; cannot resolve the GAL runtime checkout."
    }

    return Split-Path -Parent $PSScriptRoot
}

function Get-XmachineConfigPath {
    param([Parameter(Mandatory)][string]$RepoRoot)

    return Join-Path $RepoRoot "xmachine.config.json"
}

function Read-XmachineConfig {
    param([Parameter(Mandatory)][string]$RepoRoot)

    $configPath = Get-XmachineConfigPath -RepoRoot $RepoRoot
    if (-not (Test-Path $configPath)) {
        throw "Missing xmachine config '$configPath'."
    }

    try {
        $config = Get-Content $configPath -Raw | ConvertFrom-Json -AsHashtable
    }
    catch {
        throw "Invalid JSON in '$configPath'. $($_.Exception.Message)"
    }

    if ($null -eq $config -or -not $config.ContainsKey('nodes')) {
        throw "Xmachine config '$configPath' must define a top-level 'nodes' object."
    }

    $nodes = $config['nodes']
    if ($nodes -isnot [System.Collections.IDictionary]) {
        throw "Xmachine config '$configPath' must define 'nodes' as an object keyed by work-node alias."
    }

    return @{
        Path = $configPath
        Nodes = $nodes
    }
}

function Get-XmachineNodeRecord {
    param(
        [Parameter(Mandatory)][string]$RequestedNode,
        [Parameter(Mandatory)][System.Collections.IDictionary]$Nodes,
        [Parameter(Mandatory)][string]$ConfigPath
    )

    if (-not $Nodes.Contains($RequestedNode)) {
        $availableAliases = ($Nodes.Keys | Sort-Object) -join ", "
        throw "Unknown work node alias '$RequestedNode' in '$ConfigPath'. Available aliases: $availableAliases"
    }

    $nodeRecord = $Nodes[$RequestedNode]
    if ($nodeRecord -isnot [System.Collections.IDictionary]) {
        throw "Work node '$RequestedNode' in '$ConfigPath' must be an object with at least a 'target' property."
    }

    return $nodeRecord
}

function Resolve-WorkNodeTarget {
    param(
        [Parameter(Mandatory)][string]$RequestedNode,
        [Parameter(Mandatory)][System.Collections.IDictionary]$NodeRecord,
        [Parameter(Mandatory)][string]$ConfigPath
    )

    $target = if ($NodeRecord.Contains('target')) { [string]$NodeRecord['target'] } else { $null }
    if ([string]::IsNullOrWhiteSpace($target)) {
        throw "Work node '$RequestedNode' in '$ConfigPath' must define a non-empty 'target' value."
    }

    return $target.Trim()
}

function Get-ConfiguredWorkRepoPath {
    param([Parameter(Mandatory)][System.Collections.IDictionary]$NodeRecord)

    if (-not $NodeRecord.Contains('repoPath')) {
        return $null
    }

    $repoPath = [string]$NodeRecord['repoPath']
    if ([string]::IsNullOrWhiteSpace($repoPath)) {
        return $null
    }

    return $repoPath.Trim()
}

function Get-RepoMappingKey {
    param([Parameter(Mandatory)][string]$RepoContextRoot)

    return Split-Path $RepoContextRoot -Leaf
}

function Get-ConfiguredRepoMapping {
    param(
        [Parameter(Mandatory)][System.Collections.IDictionary]$NodeRecord,
        [Parameter(Mandatory)][string]$RepoKey
    )

    if (-not $NodeRecord.Contains('repoMappings')) {
        return $null
    }

    $repoMappings = $NodeRecord['repoMappings']
    if ($repoMappings -isnot [System.Collections.IDictionary] -or -not $repoMappings.Contains($RepoKey)) {
        return $null
    }

    $mapping = $repoMappings[$RepoKey]
    if ($mapping -is [string]) {
        return @{
            repoPath = $mapping
            runtimeRepoPath = $null
        }
    }

    if ($mapping -isnot [System.Collections.IDictionary]) {
        return $null
    }

    return $mapping
}

function Get-ConfiguredRuntimeRepoPath {
    param([Parameter(Mandatory)][System.Collections.IDictionary]$NodeRecord)

    if (-not $NodeRecord.Contains('runtimeRepoPath')) {
        return $null
    }

    $runtimeRepoPath = [string]$NodeRecord['runtimeRepoPath']
    if ([string]::IsNullOrWhiteSpace($runtimeRepoPath)) {
        return $null
    }

    return $runtimeRepoPath.Trim()
}

function Resolve-WorkPlatform {
    param([Parameter(Mandatory)][string]$NodeId)

    & ssh -o BatchMode=yes $NodeId "cmd /c ver" 2>&1 | Out-Null
    if ($LASTEXITCODE -eq 0) {
        return 'windows'
    }

    return 'posix'
}

function ConvertTo-RemotePowerShellLiteral {
    param([Parameter(Mandatory)][string]$Value)

    return "'" + $Value.Replace("'", "''") + "'"
}

function New-RunId {
    $datePart = Get-Date -Format 'yyyyMMdd'
    $randPart = -join ((65..90) + (97..122) + (48..57) | Get-Random -Count 6 | ForEach-Object { [char]$_ })
    return "$datePart-$randPart"
}

function Get-PlanSnapshotHash([string]$PlanPath) {
    return (Get-FileHash -Algorithm SHA256 -Path $PlanPath).Hash.ToLowerInvariant()
}

function Update-StateExecutionContext {
    param(
        [Parameter(Mandatory)][string]$StatePath,
        [Parameter(Mandatory)][string]$DispatchedNode,
        [Parameter(Mandatory)][string]$ExecutionMode,
        [Parameter(Mandatory)][string]$Notes
    )

    $stateContent = Get-Content -Path $StatePath -Raw
    $stateContent = [regex]::Replace($stateContent, '(?m)^Dispatched node:.*$', "Dispatched node: $DispatchedNode")
    $stateContent = [regex]::Replace($stateContent, '(?m)^Execution mode:.*$', "Execution mode: $ExecutionMode")
    $stateContent = [regex]::Replace($stateContent, '(?m)^Notes:.*$', "Notes: $Notes")
    Set-Content -Path $StatePath -Value $stateContent
}

function Get-RemotePipelinePaths {
    param(
        [Parameter(Mandatory)][string]$Platform,
        [Parameter(Mandatory)][string]$RunId,
        [Parameter(Mandatory)][string]$RemoteProjectRepoPath,
        [Parameter(Mandatory)][string]$RemoteRuntimeRepoPath
    )

    if ($Platform -eq 'windows') {
        $outputDir = "C:\Windows\Temp\gal-xmachine-pipeline\$RunId"
        return @{
            OutputDir = $outputDir
            PlanSnapshotPath = Join-Path $outputDir 'plan.prompt.md'
            StateSnapshotPath = Join-Path $outputDir 'state.md'
            PipelineStatusPath = Join-Path $outputDir 'pipeline-status.json'
            PipelineEventsPath = Join-Path $outputDir 'pipeline-events.jsonl'
            WorktreePath = "$RemoteProjectRepoPath-xpipeline-$RunId"
            RunnerPath = Join-Path $RemoteRuntimeRepoPath 'scripts\Start-XmachinePipeline.ps1'
            SessionName = "pipeline-$RunId"
            Launcher = 'start-process'
        }
    }

    $outputDir = "/tmp/gal-xmachine-pipeline/$RunId"
    return @{
        OutputDir = $outputDir
        PlanSnapshotPath = "$outputDir/plan.prompt.md"
        StateSnapshotPath = "$outputDir/state.md"
        PipelineStatusPath = "$outputDir/pipeline-status.json"
        PipelineEventsPath = "$outputDir/pipeline-events.jsonl"
        WorktreePath = "$RemoteProjectRepoPath-xpipeline-$RunId"
        RunnerPath = "$RemoteRuntimeRepoPath/scripts/Start-XmachinePipeline.sh"
        SessionName = "pipeline-$RunId"
        Launcher = 'zellij'
    }
}

function Invoke-RemotePowerShell {
    param(
        [Parameter(Mandatory)][string]$NodeTarget,
        [Parameter(Mandatory)][string]$Command
    )

    return (& ssh -o BatchMode=yes $NodeTarget "pwsh -NoProfile -Command `"$Command`"" 2>&1)
}

function Start-PosixPipelineRun {
    param(
        [Parameter(Mandatory)][string]$NodeTarget,
        [Parameter(Mandatory)][hashtable]$RemotePaths,
        [Parameter(Mandatory)][string]$RemoteProjectRepoPath,
        [Parameter(Mandatory)][string]$RunId,
        [Parameter(Mandatory)][string]$LocalPlanSnapshotPath,
        [Parameter(Mandatory)][string]$LocalStateSnapshotPath,
        [Parameter(Mandatory)][int]$TimeoutMinutes
    )

    Write-Host "[1/5] Creating remote output directory..."
    $mkdirResult = & ssh -o BatchMode=yes $NodeTarget "mkdir -p '$($RemotePaths.OutputDir)'" 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to create the remote pipeline output directory. $($mkdirResult -join [Environment]::NewLine)"
    }

    Write-Host "[2/5] Copying frozen plan snapshot..."
    & scp -o BatchMode=yes -q $LocalPlanSnapshotPath "${NodeTarget}:$($RemotePaths.PlanSnapshotPath)"
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to copy the plan snapshot to the work node."
    }

    & scp -o BatchMode=yes -q $LocalStateSnapshotPath "${NodeTarget}:$($RemotePaths.StateSnapshotPath)"
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to copy the state snapshot to the work node."
    }

    Write-Host "[3/5] Creating remote disposable worktree..."
    $worktreeCreateResult = & ssh -o BatchMode=yes $NodeTarget "git -C '$RemoteProjectRepoPath' -c filter.gal-config.smudge=cat -c filter.gal-config.clean=cat worktree add --detach '$($RemotePaths.WorktreePath)' HEAD" 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to create the remote pipeline worktree. $($worktreeCreateResult -join [Environment]::NewLine)"
    }

    Write-Host "[4/5] Verifying remote runner..."
    & ssh -o BatchMode=yes $NodeTarget "test -f '$($RemotePaths.RunnerPath)'" 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "Remote pipeline runner not found at '$($RemotePaths.RunnerPath)'. Pass -RemoteRuntimeRepoPath if the GAL runtime checkout lives elsewhere on the work node."
    }

    Write-Host "[5/5] Starting detached pipeline runner..."
    $remoteBootstrap = @"
set -euo pipefail
mkdir -p '$($RemotePaths.OutputDir)'
script -q /dev/null zellij attach --create-background '$($RemotePaths.SessionName)' </dev/null >/dev/null 2>&1 &
for _ in 1 2 3 4 5 6 7 8 9 10; do
  if zellij list-sessions 2>/dev/null | grep -q -F '$($RemotePaths.SessionName)'; then
    break
  fi
  sleep 0.3
done
if ! zellij list-sessions 2>/dev/null | grep -q -F '$($RemotePaths.SessionName)'; then
  exit 41
fi
zellij --session '$($RemotePaths.SessionName)' run --close-on-exit -- \
  bash '$($RemotePaths.RunnerPath)' \
    --run-id '$RunId' \
    --worktree '$($RemotePaths.WorktreePath)' \
    --plan-snapshot '$($RemotePaths.PlanSnapshotPath)' \
    --state-snapshot '$($RemotePaths.StateSnapshotPath)' \
    --output-dir '$($RemotePaths.OutputDir)' \
    --timeout-minutes '$TimeoutMinutes' \
    >/dev/null 2>&1
"@
    $bootstrapResult = & ssh -o BatchMode=yes $NodeTarget $remoteBootstrap 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to start the detached pipeline runner. $($bootstrapResult -join [Environment]::NewLine)"
    }
}

function Start-WindowsPipelineRun {
    param(
        [Parameter(Mandatory)][string]$NodeTarget,
        [Parameter(Mandatory)][hashtable]$RemotePaths,
        [Parameter(Mandatory)][string]$RemoteProjectRepoPath,
        [Parameter(Mandatory)][string]$RunId,
        [Parameter(Mandatory)][string]$LocalPlanSnapshotPath,
        [Parameter(Mandatory)][string]$LocalStateSnapshotPath,
        [Parameter(Mandatory)][int]$TimeoutMinutes
    )

    $outputDirLiteral = ConvertTo-RemotePowerShellLiteral $RemotePaths.OutputDir
    $planSnapshotLiteral = ConvertTo-RemotePowerShellLiteral $RemotePaths.PlanSnapshotPath
    $stateSnapshotLiteral = ConvertTo-RemotePowerShellLiteral $RemotePaths.StateSnapshotPath
    $worktreeLiteral = ConvertTo-RemotePowerShellLiteral $RemotePaths.WorktreePath
    $repoPathLiteral = ConvertTo-RemotePowerShellLiteral $RemoteProjectRepoPath
    $runnerLiteral = ConvertTo-RemotePowerShellLiteral $RemotePaths.RunnerPath
    $runIdLiteral = ConvertTo-RemotePowerShellLiteral $RunId
    $timeoutLiteral = ConvertTo-RemotePowerShellLiteral ([string]$TimeoutMinutes)

    Write-Host "[1/5] Creating remote output directory..."
    $mkdirResult = Invoke-RemotePowerShell -NodeTarget $NodeTarget -Command "New-Item -ItemType Directory -Force -Path $outputDirLiteral | Out-Null"
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to create the remote pipeline output directory. $($mkdirResult -join [Environment]::NewLine)"
    }

    Write-Host "[2/5] Copying frozen plan snapshot..."
    & scp -o BatchMode=yes -q $LocalPlanSnapshotPath "${NodeTarget}:$($RemotePaths.PlanSnapshotPath)"
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to copy the plan snapshot to the work node."
    }

    & scp -o BatchMode=yes -q $LocalStateSnapshotPath "${NodeTarget}:$($RemotePaths.StateSnapshotPath)"
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to copy the state snapshot to the work node."
    }

    Write-Host "[3/5] Creating remote disposable worktree..."
    $worktreeCreateResult = Invoke-RemotePowerShell -NodeTarget $NodeTarget -Command "git -C $repoPathLiteral -c filter.gal-config.smudge=cat -c filter.gal-config.clean=cat worktree add --detach $worktreeLiteral HEAD"
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to create the remote pipeline worktree. $($worktreeCreateResult -join [Environment]::NewLine)"
    }

    Write-Host "[4/5] Verifying remote runner..."
    Invoke-RemotePowerShell -NodeTarget $NodeTarget -Command "if (-not (Test-Path $runnerLiteral)) { exit 41 }" | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "Remote pipeline runner not found at '$($RemotePaths.RunnerPath)'. Pass -RemoteRuntimeRepoPath if the GAL runtime checkout lives elsewhere on the work node."
    }

    Write-Host "[5/5] Starting detached pipeline runner..."
    $runtimeCmd = "Start-Process pwsh -ArgumentList '-NoProfile','-File',$runnerLiteral,'-RunId',$runIdLiteral,'-WorktreePath',$worktreeLiteral,'-PlanSnapshot',$planSnapshotLiteral,'-StateSnapshot',$stateSnapshotLiteral,'-OutputDir',$outputDirLiteral,'-TimeoutMinutes',$timeoutLiteral -WindowStyle Hidden"
    $bootstrapResult = Invoke-RemotePowerShell -NodeTarget $NodeTarget -Command $runtimeCmd
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to start the detached pipeline runner. $($bootstrapResult -join [Environment]::NewLine)"
    }
}

$runtimeRepoRoot = Get-RepoRoot
$repoContextRoot = Get-RepoContextRoot
$statePath = Get-StatePath -RepoContextRoot $repoContextRoot
$resolvedPlanPath = if ($PlanPath) {
    Resolve-PlanPath -Value $PlanPath -RepoContextRoot $repoContextRoot
}
else {
    Get-ActivePlanPath -RepoContextRoot $repoContextRoot
}

if ([string]::IsNullOrWhiteSpace($resolvedPlanPath) -or -not (Test-Path $resolvedPlanPath)) {
    throw "Could not resolve the active plan file for xmachine pipeline dispatch. Pass -PlanPath explicitly or update .dev/state.md Active Plans."
}

$xmachineConfig = Read-XmachineConfig -RepoRoot $runtimeRepoRoot
$workNodeRecord = Get-XmachineNodeRecord -RequestedNode $WorkNode -Nodes $xmachineConfig.Nodes -ConfigPath $xmachineConfig.Path
$resolvedWorkNodeTarget = Resolve-WorkNodeTarget -RequestedNode $WorkNode -NodeRecord $workNodeRecord -ConfigPath $xmachineConfig.Path
$repoMappingKey = Get-RepoMappingKey -RepoContextRoot $repoContextRoot
$repoMapping = Get-ConfiguredRepoMapping -NodeRecord $workNodeRecord -RepoKey $repoMappingKey
$mappedProjectRepoPath = if ($null -ne $repoMapping -and $repoMapping.Contains('repoPath')) { [string]$repoMapping['repoPath'] } else { $null }
$mappedRuntimeRepoPath = if ($null -ne $repoMapping -and $repoMapping.Contains('runtimeRepoPath')) { [string]$repoMapping['runtimeRepoPath'] } else { $null }
$resolvedProjectRepoPath = if ($WorkRepoPath) { $WorkRepoPath } elseif (-not [string]::IsNullOrWhiteSpace($mappedProjectRepoPath)) { $mappedProjectRepoPath.Trim() } else { Get-ConfiguredWorkRepoPath -NodeRecord $workNodeRecord }

if ([string]::IsNullOrWhiteSpace($resolvedProjectRepoPath)) {
    throw "No repo path is configured for work node '$WorkNode'. Pass -WorkRepoPath or define 'repoMappings.$repoMappingKey.repoPath' or 'repoPath' in '$($xmachineConfig.Path)'."
}

$resolvedRemoteRuntimeRepoPath = if ($RemoteRuntimeRepoPath) { $RemoteRuntimeRepoPath } elseif (-not [string]::IsNullOrWhiteSpace($mappedRuntimeRepoPath)) { $mappedRuntimeRepoPath.Trim() } else { Get-ConfiguredRuntimeRepoPath -NodeRecord $workNodeRecord }
if ([string]::IsNullOrWhiteSpace($resolvedRemoteRuntimeRepoPath)) {
    $resolvedRemoteRuntimeRepoPath = $resolvedProjectRepoPath
}
$resolvedPlatform = Resolve-WorkPlatform -NodeId $resolvedWorkNodeTarget
$runId = New-RunId

$branchName = (& git -C $repoContextRoot rev-parse --abbrev-ref HEAD 2>$null | Select-Object -First 1)
if ($LASTEXITCODE -ne 0) {
    throw "Could not determine the current branch from '$repoContextRoot'."
}

$baseCommit = (& git -C $repoContextRoot rev-parse HEAD 2>$null | Select-Object -First 1)
if ($LASTEXITCODE -ne 0) {
    throw "Could not determine the current HEAD commit from '$repoContextRoot'."
}

$planHash = Get-PlanSnapshotHash -PlanPath $resolvedPlanPath
$runRecordsDir = Join-Path $repoContextRoot '.dev\xmachine-runs'
$localStagingDir = Join-Path $runRecordsDir $runId
$localRunRecordPath = Join-Path $runRecordsDir "$runId.json"
$localPlanSnapshotPath = Join-Path $localStagingDir 'plan.prompt.md'
$localStateSnapshotPath = Join-Path $localStagingDir 'state.md'

New-Item -ItemType Directory -Force -Path $localStagingDir | Out-Null
Copy-Item -Path $resolvedPlanPath -Destination $localPlanSnapshotPath -Force
Copy-Item -Path $statePath -Destination $localStateSnapshotPath -Force

$remotePaths = Get-RemotePipelinePaths -Platform $resolvedPlatform -RunId $runId -RemoteProjectRepoPath $resolvedProjectRepoPath -RemoteRuntimeRepoPath $resolvedRemoteRuntimeRepoPath

$runRecord = [ordered]@{
    runId = $runId
    status = 'dispatched'
    workNode = $WorkNode
    target = $resolvedWorkNodeTarget
    platform = $resolvedPlatform
    targetProjectRoot = $repoContextRoot
    localPlanPath = $resolvedPlanPath
    planSnapshotHash = $planHash
    branch = $branchName.Trim()
    baseCommit = $baseCommit.Trim()
    remoteProjectRepoPath = $resolvedProjectRepoPath
    remoteRuntimeRepoPath = $resolvedRemoteRuntimeRepoPath
    remoteOutputDir = $remotePaths.OutputDir
    remoteWorktreePath = $remotePaths.WorktreePath
    remotePipelineStatusPath = $remotePaths.PipelineStatusPath
    remotePipelineEventsPath = $remotePaths.PipelineEventsPath
    remoteSessionName = $remotePaths.SessionName
    launcher = $remotePaths.Launcher
    dispatchTimeUtc = (Get-Date).ToUniversalTime().ToString('o')
    convergeCommand = "/gal status"
}
$runRecord | ConvertTo-Json -Depth 6 | Set-Content -Path $localRunRecordPath -Encoding UTF8

Update-StateExecutionContext -StatePath $statePath -DispatchedNode $WorkNode -ExecutionMode 'xmachine-persistent' -Notes ("Active run: {0}; plan: {1}; status: dispatched" -f $runId, (Split-Path -Leaf $resolvedPlanPath))

Write-Host "GAL Remote Pipeline Run: $runId"
Write-Host "  Machine:  $resolvedWorkNodeTarget"
Write-Host "  Project:  $resolvedProjectRepoPath"
Write-Host "  Runtime:  $resolvedRemoteRuntimeRepoPath"
Write-Host "  Plan:     $resolvedPlanPath"
Write-Host "  Platform: $resolvedPlatform"
Write-Host "  Branch:   $($branchName.Trim())"
Write-Host "  Base:     $($baseCommit.Trim())"
Write-Host ""

switch ($resolvedPlatform) {
    'windows' {
        Start-WindowsPipelineRun -NodeTarget $resolvedWorkNodeTarget -RemotePaths $remotePaths -RemoteProjectRepoPath $resolvedProjectRepoPath -RunId $runId -LocalPlanSnapshotPath $localPlanSnapshotPath -LocalStateSnapshotPath $localStateSnapshotPath -TimeoutMinutes $TimeoutMinutes
    }
    default {
        Start-PosixPipelineRun -NodeTarget $resolvedWorkNodeTarget -RemotePaths $remotePaths -RemoteProjectRepoPath $resolvedProjectRepoPath -RunId $runId -LocalPlanSnapshotPath $localPlanSnapshotPath -LocalStateSnapshotPath $localStateSnapshotPath -TimeoutMinutes $TimeoutMinutes
    }
}

$runRecord.status = 'running'
$runRecord.remoteSessionStartedUtc = (Get-Date).ToUniversalTime().ToString('o')
$runRecord | ConvertTo-Json -Depth 6 | Set-Content -Path $localRunRecordPath -Encoding UTF8
Update-StateExecutionContext -StatePath $statePath -DispatchedNode $WorkNode -ExecutionMode 'xmachine-persistent' -Notes ("Active run: {0}; session: {1}; status: running" -f $runId, $remotePaths.SessionName)

if (-not $KeepRemote) {
    Write-Host ""
    Write-Host "Remote cleanup is deferred until convergence."
}

Write-Host ""
Write-Host "Persistent pipeline dispatched successfully."
Write-Host ""
Write-Host "  Run ID:        $runId"
Write-Host "  Session:       $($remotePaths.SessionName)"
Write-Host "  Remote output: $($remotePaths.OutputDir)"
Write-Host "  Run record:    $localRunRecordPath"
Write-Host ""
Write-Host "Next-day recovery entry point:"
Write-Host "  /gal status"
