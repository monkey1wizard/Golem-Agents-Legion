<#
.SYNOPSIS
    Dispatch a task spec to a configured xmachine work node from a Windows control node.

.DESCRIPTION
    Resolves a work-node alias from the canonical ~/.gal/config/xmachine.json path,
    with warned repository-root fallback during migration, detects the remote platform,
    dispatches the task through the existing per-lane xmachine scripts, and optionally
    retrieves the standard runtime artifacts into a local output directory.
#>

param(
    [Parameter(Mandatory)]
    [string]$WorkNode,

    [Parameter(Mandatory)]
    [string]$TaskSpec,

    [string]$WorkRepoPath,

    [string]$RemoteRuntimeRepoPath,

    [ValidateSet("auto", "posix", "windows")]
    [string]$WorkPlatform = "auto",

    [int]$TimeoutMinutes = 30,

    [switch]$Wait,

    [string]$LocalOutputDir,

    [switch]$KeepRemote
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot 'common\Common.ps1')

function Get-RepoRoot {
    if ([string]::IsNullOrWhiteSpace($PSScriptRoot)) {
        throw "PSScriptRoot is not available; cannot resolve the repository root."
    }

    return Split-Path -Parent $PSScriptRoot
}

function Get-RepoContextRoot {
    $current = (Get-Location).Path

    while ($true) {
        $statePath = Join-Path $current ".dev\state.md"
        if (Test-Path $statePath) {
            return $current
        }

        $parent = Split-Path $current -Parent
        if ([string]::IsNullOrWhiteSpace($parent) -or $parent -eq $current) {
            return (Get-Location).Path
        }

        $current = $parent
    }
}

function Get-XmachineConfigPath {
    param([Parameter(Mandatory)][string]$RepoRoot)

    return (Resolve-XmachineConfigRecord -RepoRoot $RepoRoot -WarnOnLegacyFallback).Path
}

function Read-XmachineConfig {
    param([Parameter(Mandatory)][string]$RepoRoot)

    $configPath = Get-XmachineConfigPath -RepoRoot $RepoRoot
    if (-not (Test-Path $configPath)) {
        throw "Missing xmachine config '$configPath'."
    }

    $raw = Get-Content $configPath -Raw
    if ([string]::IsNullOrWhiteSpace($raw)) {
        throw "Xmachine config '$configPath' is empty."
    }

    $config = Read-JsonOrderedMap $configPath
    if ($null -eq $config) {
        throw "Invalid JSON in '$configPath'."
    }

    $nodesKey = if ($config.Contains('xmachineNodeAliases')) { 'xmachineNodeAliases' } elseif ($config.Contains('nodes')) { 'nodes' } else { $null }
    if ($null -eq $nodesKey) {
        throw "Xmachine config '$configPath' must define top-level 'xmachineNodeAliases' or legacy 'nodes'."
    }

    $nodes = $config[$nodesKey]
    if ($nodes -isnot [System.Collections.IDictionary]) {
        throw "Xmachine config '$configPath' must define '$nodesKey' as an object keyed by work-node alias."
    }

    return @{
        path = $configPath
        nodes = $nodes
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

    $target = if ($NodeRecord.Contains("target")) { [string]$NodeRecord["target"] } else { $null }
    if ([string]::IsNullOrWhiteSpace($target)) {
        throw "Work node '$RequestedNode' in '$ConfigPath' must define a non-empty 'target' value."
    }

    return $target.Trim()
}

function Get-ConfiguredWorkRepoPath {
    param([Parameter(Mandatory)][System.Collections.IDictionary]$NodeRecord)

    if (-not $NodeRecord.Contains("repoPath")) {
        return $null
    }

    $repoPath = [string]$NodeRecord["repoPath"]
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

    if (-not $NodeRecord.Contains("repoMappings")) {
        return $null
    }

    $repoMappings = $NodeRecord["repoMappings"]
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

    if (-not $NodeRecord.Contains("runtimeRepoPath")) {
        return $null
    }

    $runtimeRepoPath = [string]$NodeRecord["runtimeRepoPath"]
    if ([string]::IsNullOrWhiteSpace($runtimeRepoPath)) {
        return $null
    }

    return $runtimeRepoPath.Trim()
}

function ConvertFrom-SshConfigOutput {
    param([Parameter(Mandatory)][string[]]$Lines)

    $map = @{}
    foreach ($line in $Lines) {
        if ([string]::IsNullOrWhiteSpace($line)) {
            continue
        }

        $parts = $line -split '\s+', 2
        if ($parts.Count -ne 2) {
            continue
        }

        $map[$parts[0].ToLowerInvariant()] = $parts[1].Trim()
    }

    return $map
}

function Resolve-WorkPlatform {
    param(
        [Parameter(Mandatory)][string]$Platform,
        [Parameter(Mandatory)][string]$NodeId
    )

    if ($Platform -ne "auto") {
        return $Platform
    }

    & ssh -o BatchMode=yes $NodeId "cmd /c ver" 2>&1 | Out-Null
    if ($LASTEXITCODE -eq 0) {
        return "windows"
    }

    return "posix"
}

function Invoke-SshCommand {
    param(
        [Parameter(Mandatory)][string]$NodeId,
        [Parameter(Mandatory)][string]$RemoteCommand
    )

    $output = & ssh -o BatchMode=yes $NodeId $RemoteCommand 2>&1
    return @{
        ExitCode = $LASTEXITCODE
        Output = @($output | ForEach-Object { $_.ToString() })
    }
}

function Invoke-PosixCommand {
    param(
        [Parameter(Mandatory)][string]$NodeId,
        [Parameter(Mandatory)][string]$Script
    )

    $bootstrap = "export PATH=/opt/homebrew/bin:/usr/local/bin:`$HOME/.local/bin:`$PATH; source ~/.zprofile >/dev/null 2>&1 || true; source ~/.zshrc >/dev/null 2>&1 || true; "
    return Invoke-SshCommand -NodeId $NodeId -RemoteCommand ($bootstrap + $Script)
}

function Copy-PosixRuntimeScripts {
    param(
        [Parameter(Mandatory)][string]$SshTarget,
        [Parameter(Mandatory)][string]$RemoteRuntimeStagePath
    )

    $remoteScriptsDir = "$RemoteRuntimeStagePath/scripts"
    $mkdirResult = Invoke-PosixCommand -NodeId $SshTarget -Script "mkdir -p '$remoteScriptsDir'"
    if ($mkdirResult.ExitCode -ne 0) {
        throw "Failed to create staged xmachine runtime directory '$remoteScriptsDir'.`n$($mkdirResult.Output -join [Environment]::NewLine)"
    }

    foreach ($scriptName in @("Invoke-XmachineLocalTask.sh", "Start-xMachine.sh", "Get-XmachineLocalResult.sh")) {
        $localScript = Join-Path $PSScriptRoot $scriptName
        if (-not (Test-Path $localScript)) {
            throw "Missing local xmachine runtime script '$localScript'."
        }

        & scp -o BatchMode=yes -q $localScript "${SshTarget}:$remoteScriptsDir/$scriptName"
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to stage xmachine runtime script '$scriptName' on '$SshTarget'."
        }
    }

    $normalizeScript = "for f in '$remoteScriptsDir'/*.sh; do tmp=`"`$f.tmp`"; tr -d '\r' < `"`$f`" > `"`$tmp`" && mv `"`$tmp`" `"`$f`" && chmod +x `"`$f`"; done"
    $normalizeResult = Invoke-PosixCommand -NodeId $SshTarget -Script $normalizeScript
    if ($normalizeResult.ExitCode -ne 0) {
        throw "Failed to normalize staged xmachine runtime scripts in '$remoteScriptsDir'.`n$($normalizeResult.Output -join [Environment]::NewLine)"
    }
}

function Get-SshConnectionInfo {
    param([Parameter(Mandatory)][string]$SshTarget)

    $sshConfigOutput = & ssh -G $SshTarget 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Could not resolve SSH target '$SshTarget'."
    }

    $sshConfig = ConvertFrom-SshConfigOutput -Lines @($sshConfigOutput | ForEach-Object { $_.ToString() })
    return @{
        Host = if ($sshConfig.ContainsKey("host")) { $sshConfig["host"] } else { $SshTarget }
        User = if ($sshConfig.ContainsKey("user")) { $sshConfig["user"] } else { $null }
    }
}

function New-TaskId {
    $datePart = Get-Date -Format "yyyyMMdd"
    $randPart = -join ((65..90) + (97..122) + (48..57) | Get-Random -Count 6 | ForEach-Object { [char]$_ })
    return "$datePart-$randPart"
}

function Wait-ForPosixTaskCompletion {
    param(
        [Parameter(Mandatory)][string]$SshTarget,
        [Parameter(Mandatory)][string]$RemoteOutputDir,
        [Parameter(Mandatory)][string]$TaskId,
        [Parameter(Mandatory)][int]$TimeoutMinutes
    )

    $remoteStatusPath = "$RemoteOutputDir/status.json"
    $deadline = (Get-Date).AddMinutes($TimeoutMinutes)
    Write-Host "Waiting for task $TaskId to complete (timeout: ${TimeoutMinutes}m)..."

    while ((Get-Date) -lt $deadline) {
        $statusResult = Invoke-SshCommand -NodeId $SshTarget -RemoteCommand "cat '$remoteStatusPath' 2>/dev/null"
        $rawStatus = $statusResult.Output
        if ($statusResult.ExitCode -eq 0 -and $rawStatus) {
            try {
                $statusObj = $rawStatus | ConvertFrom-Json
                if ($statusObj.status -ne "running") {
                    Write-Host "Task $TaskId — status: $($statusObj.status)"
                    return
                }
            }
            catch { }
        }

        Write-Host "  Still running... (checking again in 30s)"
        Start-Sleep -Seconds 30
    }

    Write-Warning "Timed out waiting for task $TaskId. Retrieving partial results."
}

function Receive-PosixTaskArtifacts {
    param(
        [Parameter(Mandatory)][string]$SshTarget,
        [Parameter(Mandatory)][string]$TaskId,
        [Parameter(Mandatory)][string]$RemoteOutputDir,
        [Parameter(Mandatory)][AllowEmptyString()][string]$RemoteProjectRepoPath,
        [Parameter(Mandatory)][string]$RemoteRuntimeRepoPath,
        [Parameter(Mandatory)][string]$LocalOutputDir,
        [string]$RemoteRuntimeStagePath = "",
        [switch]$KeepRemote
    )

    New-Item -ItemType Directory -Force -Path $LocalOutputDir | Out-Null

    Write-Host "Retrieving results for task $TaskId..."
    Write-Host "  From: ${SshTarget}:$RemoteOutputDir"
    Write-Host "  To:   $LocalOutputDir"

    $outputFiles = @("status.json", "summary.md", "runtime.log", "result.patch")
    foreach ($outputFile in $outputFiles) {
        $remotePath = "$RemoteOutputDir/$outputFile"
        $localPath = Join-Path $LocalOutputDir $outputFile
        & scp -o BatchMode=yes -q "${SshTarget}:$remotePath" $localPath 2>$null | Out-Null
    }

    $localStatusPath = Join-Path $LocalOutputDir "status.json"
    $localSummaryPath = Join-Path $LocalOutputDir "summary.md"
    $localPatchPath = Join-Path $LocalOutputDir "result.patch"

    if (Test-Path $localStatusPath) {
        Write-Host ""
        Write-Host "─── Task Status ───────────────────────────────────────────"
        $statusObj = Get-Content $localStatusPath | ConvertFrom-Json
        Write-Host "  Task ID:    $($statusObj.taskId)"
        Write-Host "  Status:     $($statusObj.status)"
        Write-Host "  Exit Code:  $($statusObj.exitCode)"
        Write-Host "  Started:    $($statusObj.startedAt)"
        Write-Host "  Finished:   $($statusObj.finishedAt)"
        if ($statusObj.errorMessage) {
            Write-Host "  Error:      $($statusObj.errorMessage)"
        }
        Write-Host "───────────────────────────────────────────────────────────"
    }

    if (Test-Path $localSummaryPath) {
        Write-Host ""
        Write-Host "─── Summary ───────────────────────────────────────────────"
        Get-Content $localSummaryPath | Write-Host
        Write-Host "───────────────────────────────────────────────────────────"
    }

    if (Test-Path $localPatchPath) {
        $patchSize = (Get-Item $localPatchPath).Length
        Write-Host ""
        if ($patchSize -gt 0) {
            Write-Host "result.patch: $patchSize bytes — review and apply with: git apply '$localPatchPath'"
        }
        else {
            Write-Host "result.patch: empty (read-only task, no file changes)"
        }
    }

    if (-not $KeepRemote) {
        if (-not [string]::IsNullOrWhiteSpace($RemoteRuntimeStagePath)) {
            $cleanupScript = "bash '$RemoteRuntimeStagePath/scripts/Get-XmachineLocalResult.sh' --task-id '$TaskId' --output-dir '$RemoteOutputDir'"
            if (-not [string]::IsNullOrWhiteSpace($RemoteProjectRepoPath)) {
                $cleanupScript += " --repo-path '$RemoteProjectRepoPath'"
            }
            $cleanupScript += " >/dev/null; rm -rf '$RemoteRuntimeStagePath'"
        }
        else {
            $cleanupScript = "cd '$RemoteRuntimeRepoPath' && bash scripts/Get-XmachineLocalResult.sh --task-id '$TaskId' --output-dir '$RemoteOutputDir'"
            if (-not [string]::IsNullOrWhiteSpace($RemoteProjectRepoPath)) {
                $cleanupScript += " --repo-path '$RemoteProjectRepoPath'"
            }
            $cleanupScript += " >/dev/null"
        }
        & ssh -o BatchMode=yes $SshTarget $cleanupScript 2>$null | Out-Null
    }

    Write-Host ""
    Write-Host "Results saved to: $LocalOutputDir"
}

$repoRoot = Get-RepoRoot
$repoContextRoot = Get-RepoContextRoot
$xmachineConfig = Read-XmachineConfig -RepoRoot $repoRoot
$workNodeRecord = Get-XmachineNodeRecord -RequestedNode $WorkNode -Nodes $xmachineConfig.nodes -ConfigPath $xmachineConfig.path
$resolvedWorkNodeTarget = Resolve-WorkNodeTarget -RequestedNode $WorkNode -NodeRecord $workNodeRecord -ConfigPath $xmachineConfig.path
$configuredNodeRepoPath = Get-ConfiguredWorkRepoPath -NodeRecord $workNodeRecord
$repoMappingKey = Get-RepoMappingKey -RepoContextRoot $repoContextRoot
$repoMapping = Get-ConfiguredRepoMapping -NodeRecord $workNodeRecord -RepoKey $repoMappingKey
$mappedProjectRepoPath = if ($null -ne $repoMapping -and $repoMapping.Contains("repoPath")) { [string]$repoMapping["repoPath"] } else { $null }
$mappedRuntimeRepoPath = if ($null -ne $repoMapping -and $repoMapping.Contains("runtimeRepoPath")) { [string]$repoMapping["runtimeRepoPath"] } else { $null }
$resolvedProjectRepoPath = if ($WorkRepoPath) { $WorkRepoPath } elseif (-not [string]::IsNullOrWhiteSpace($mappedProjectRepoPath)) { $mappedProjectRepoPath.Trim() } else { $null }
$executionMode = if ([string]::IsNullOrWhiteSpace($resolvedProjectRepoPath)) { "execute" } else { "repo" }

$resolvedRemoteRuntimeRepoPath = if ($RemoteRuntimeRepoPath) { $RemoteRuntimeRepoPath } elseif (-not [string]::IsNullOrWhiteSpace($mappedRuntimeRepoPath)) { $mappedRuntimeRepoPath.Trim() } else { Get-ConfiguredRuntimeRepoPath -NodeRecord $workNodeRecord }
if ([string]::IsNullOrWhiteSpace($resolvedRemoteRuntimeRepoPath)) {
    $resolvedRemoteRuntimeRepoPath = $configuredNodeRepoPath
}

if ([string]::IsNullOrWhiteSpace($resolvedRemoteRuntimeRepoPath)) {
    throw "No GAL runtime path is configured for work node '$WorkNode'. Pass -RemoteRuntimeRepoPath or define 'runtimeRepoPath' or 'repoPath' in '$($xmachineConfig.path)'."
}

if (-not (Test-Path $TaskSpec)) {
    throw "Task spec not found: $TaskSpec"
}

$resolvedPlatform = Resolve-WorkPlatform -Platform $WorkPlatform -NodeId $resolvedWorkNodeTarget
$taskId = New-TaskId

if ($resolvedPlatform -eq "windows") {
    $sshConnection = Get-SshConnectionInfo -SshTarget $resolvedWorkNodeTarget
    if ([string]::IsNullOrWhiteSpace($sshConnection.User)) {
        throw "SSH config for work node '$WorkNode' does not expose a User value."
    }

    $dispatchArgs = @{
        RemoteHost = $sshConnection.Host
        RemoteUser = $sshConnection.User
        RemoteScriptsPath = (Join-Path $resolvedRemoteRuntimeRepoPath "scripts")
        TaskSpec = $TaskSpec
        TimeoutMinutes = $TimeoutMinutes
    }
    if ($executionMode -eq "repo") {
        $dispatchArgs.RemoteRepoPath = $resolvedProjectRepoPath
    }

    $dispatchOutput = & (Join-Path $PSScriptRoot "Invoke-XmachineRemoteTask.ps1") @dispatchArgs 2>&1

    $dispatchOutput | ForEach-Object { Write-Host $_ }

    $taskId = ($dispatchOutput | Select-String 'Task ID:\s+(.+)$' | Select-Object -First 1).Matches.Groups[1].Value.Trim()
    $remoteOutputDir = ($dispatchOutput | Select-String 'Remote output:\s+(.+)$' | Select-Object -First 1).Matches.Groups[1].Value.Trim()
    if (-not $taskId -or -not $remoteOutputDir) {
        throw "Could not parse task metadata from Invoke-XmachineRemoteTask output."
    }

    if ($Wait) {
        $retrieveArgs = @{
            RemoteHost = $sshConnection.Host
            RemoteUser = $sshConnection.User
            TaskId = $taskId
            RemoteOutputDir = $remoteOutputDir
            RemoteRepoPath = $resolvedProjectRepoPath
            Wait = $true
            TimeoutMinutes = [Math]::Max($TimeoutMinutes, 60)
            KeepRemote = $KeepRemote
        }

        if ($LocalOutputDir) {
            $retrieveArgs.LocalOutputDir = $LocalOutputDir
        }

        & (Join-Path $PSScriptRoot "Get-XmachineRemoteResult.ps1") @retrieveArgs
    }

    exit 0
}

$taskSpecName = Split-Path -Leaf $TaskSpec
$remoteTaskSpec = "/tmp/gal-xmachine-task-$taskId-$taskSpecName"
$remoteRuntimeStagePath = if ($executionMode -eq "execute") { "/tmp/gal-xmachine-runtime-$taskId" } else { "" }
$dispatchRuntimePath = if ($executionMode -eq "execute") { $remoteRuntimeStagePath } else { $resolvedRemoteRuntimeRepoPath }

Write-Host "GAL Remote Task: $taskId"
Write-Host "  Machine:  $resolvedWorkNodeTarget"
if ($executionMode -eq "repo") {
    Write-Host "  Project:  $resolvedProjectRepoPath"
}
else {
    Write-Host "  Project:  (execute mode; temp workspace only)"
}
Write-Host "  Runtime:  $resolvedRemoteRuntimeRepoPath"
Write-Host "  TaskSpec: $taskSpecName"
Write-Host "  Timeout:  ${TimeoutMinutes}m"
Write-Host ""

Write-Host "[1/3] Copying task spec to remote..."
& scp -o BatchMode=yes -q $TaskSpec "${resolvedWorkNodeTarget}:$remoteTaskSpec"
if ($LASTEXITCODE -ne 0) {
    throw "Failed to copy task spec to POSIX work node '$resolvedWorkNodeTarget'."
}

if ($executionMode -eq "execute") {
    Write-Host "[1/3] Staging execute-mode xmachine runtime scripts..."
    Copy-PosixRuntimeScripts -SshTarget $resolvedWorkNodeTarget -RemoteRuntimeStagePath $remoteRuntimeStagePath
}

Write-Host "[2/3] Dispatching task on POSIX work node..."
$dispatchScript = "cd '$dispatchRuntimePath' && bash scripts/Invoke-XmachineLocalTask.sh --task-spec '$remoteTaskSpec' --timeout-minutes $TimeoutMinutes --task-id '$taskId'"
if ($executionMode -eq "repo") {
    $dispatchScript += " --repo-path '$resolvedProjectRepoPath'"
}
$dispatchResult = Invoke-PosixCommand -NodeId $resolvedWorkNodeTarget -Script $dispatchScript
$dispatchOutput = $dispatchResult.Output
if ($dispatchResult.ExitCode -ne 0) {
    $failedDispatchCleanup = "rm -f '$remoteTaskSpec'"
    if (-not [string]::IsNullOrWhiteSpace($remoteRuntimeStagePath)) {
        $failedDispatchCleanup += "; rm -rf '$remoteRuntimeStagePath'"
    }
    Invoke-PosixCommand -NodeId $resolvedWorkNodeTarget -Script $failedDispatchCleanup | Out-Null
    throw "Failed to dispatch task on POSIX work node '$resolvedWorkNodeTarget'.`n$($dispatchOutput -join [Environment]::NewLine)"
}

$dispatchOutput | ForEach-Object { Write-Host $_ }

$remoteOutputDir = ($dispatchOutput | Select-String '^[ ]*Output:\s+(.+)$' | Select-Object -First 1).Matches.Groups[1].Value.Trim()
if (-not $remoteOutputDir) {
    throw "Could not parse the POSIX output directory from dispatch output."
}

Write-Host "[3/3] Cleaning up staged remote task spec..."
Invoke-PosixCommand -NodeId $resolvedWorkNodeTarget -Script "rm -f '$remoteTaskSpec'" | Out-Null

Write-Host ""
Write-Host "Task dispatched successfully."
Write-Host ""
Write-Host "  Task ID:        $taskId"
Write-Host "  Remote output:  $remoteOutputDir"
Write-Host ""

if (-not $Wait) {
    Write-Host "Retrieve results when done:"
    Write-Host "  .\scripts\Invoke-XmachineTask.ps1 -WorkNode $WorkNode -TaskSpec '$TaskSpec' -Wait"
    exit 0
}

$resolvedLocalOutputDir = if ($LocalOutputDir) { $LocalOutputDir } else { Join-Path (Get-Location) "gal-results\$taskId" }
Wait-ForPosixTaskCompletion -SshTarget $resolvedWorkNodeTarget -RemoteOutputDir $remoteOutputDir -TaskId $taskId -TimeoutMinutes ([Math]::Max($TimeoutMinutes, 60))
Receive-PosixTaskArtifacts -SshTarget $resolvedWorkNodeTarget -TaskId $taskId -RemoteOutputDir $remoteOutputDir -RemoteProjectRepoPath $resolvedProjectRepoPath -RemoteRuntimeRepoPath $resolvedRemoteRuntimeRepoPath -LocalOutputDir $resolvedLocalOutputDir -RemoteRuntimeStagePath $remoteRuntimeStagePath -KeepRemote:$KeepRemote
