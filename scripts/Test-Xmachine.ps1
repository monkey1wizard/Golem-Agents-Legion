<#!
.SYNOPSIS
    Validate and smoke-test an xmachine work node from a Windows control node.

.DESCRIPTION
    Resolves a user-facing xmachine work-node alias into a concrete SSH target,
    verifies staged readiness from the Windows control node, runs the current
    work-node smoke path plus a GAL pipeline dispatch smoke gate, and writes the
    resulting readiness state to `~/.gal/xmachine-nodes.json` so later repos can
    reuse already verified nodes without rediscovering them.

.PARAMETER WorkNode
    User-facing xmachine work-node alias. The alias must exist in
    `~/.gal/config/xmachine.json`, with warned repository-root fallback during migration.

.PARAMETER WorkRepoPath
    Repo checkout path on the work node. If omitted, the script tries to reuse
    the last cached repo path for this repo from `~/.gal/xmachine-nodes.json`.

.PARAMETER WorkPlatform
    Optional work-node platform override. Defaults to `auto`.
#>

param(
    [Alias("RemoteHost")]
    [string]$WorkNode,

    [Alias("RemoteRepoPath", "MacRepoPath")]
    [string]$WorkRepoPath,

    [ValidateSet("auto", "posix", "windows")]
    [string]$WorkPlatform = "auto",

    [int]$TimeoutMinutes = 30,

    [switch]$Wait,

    [string]$LocalOutputDir
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot 'common\Common.ps1')

function Get-RepoRoot {
    $scriptDir = $PSScriptRoot
    if ([string]::IsNullOrWhiteSpace($scriptDir)) {
        throw "PSScriptRoot is not available; cannot resolve the repository root."
    }

    return Split-Path -Parent $scriptDir
}

function Get-RepoName {
    param([Parameter(Mandatory)][string]$RepoRoot)
    return Split-Path $RepoRoot -Leaf
}

function Get-XmachineConfigPath {
    param([Parameter(Mandatory)][string]$RepoRoot)

    return (Resolve-XmachineConfigRecord -RepoRoot $RepoRoot -WarnOnLegacyFallback).Path
}

function Read-XmachineConfig {
    param([Parameter(Mandatory)][string]$RepoRoot)

    $configPath = Get-XmachineConfigPath -RepoRoot $RepoRoot
    if (-not (Test-Path $configPath)) {
        throw "Missing xmachine config '$configPath'. Create ~/.gal/config/xmachine.json or keep the repository-root fallback only temporarily during migration."
    }

    $raw = Get-Content $configPath -Raw
    if ([string]::IsNullOrWhiteSpace($raw)) {
        throw "Xmachine config '$configPath' is empty. Define at least one node in ~/.gal/config/xmachine.json or the temporary repository-root fallback."
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

    if ($Nodes.Count -eq 0) {
        throw "Xmachine config '$ConfigPath' does not define any work nodes. Add entries under the top-level 'nodes' object before running xmachine smoke tests."
    }

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

    $target = $null
    if ($NodeRecord.Contains("target")) {
        $target = [string]$NodeRecord["target"]
    }

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

function Get-GalStateRoot {
    $galRoot = Join-Path $env:USERPROFILE ".gal"
    New-Item -ItemType Directory -Force -Path $galRoot | Out-Null
    return $galRoot
}

function Get-XmachineNodeCachePath {
    return Join-Path (Get-GalStateRoot) "xmachine-nodes.json"
}

function Read-XmachineNodeCache {
    $cachePath = Get-XmachineNodeCachePath
    if (-not (Test-Path $cachePath)) {
        return @{
            version = 1
            nodes = @()
        }
    }

    $raw = Get-Content $cachePath -Raw
    if ([string]::IsNullOrWhiteSpace($raw)) {
        return @{
            version = 1
            nodes = @()
        }
    }

    return ConvertTo-XmachineHashtable ($raw | ConvertFrom-Json)
}

function ConvertTo-XmachineHashtable {
    param([object]$InputObject)

    if ($null -eq $InputObject) { return $null }

    if ($InputObject -is [System.Collections.IDictionary]) {
        $hash = @{}
        foreach ($key in $InputObject.Keys) {
            $hash[$key] = ConvertTo-XmachineHashtable $InputObject[$key]
        }
        return $hash
    }

    if ($InputObject -is [System.Collections.IEnumerable] -and $InputObject -isnot [string]) {
        $items = [System.Collections.Generic.List[object]]::new()
        foreach ($item in $InputObject) {
            $items.Add((ConvertTo-XmachineHashtable $item))
        }
        return $items.ToArray()
    }

    if ($InputObject.PSObject -and $InputObject -isnot [string] -and $InputObject -isnot [ValueType]) {
        $hash = @{}
        foreach ($property in $InputObject.PSObject.Properties) {
            $hash[$property.Name] = ConvertTo-XmachineHashtable $property.Value
        }
        return $hash
    }

    return $InputObject
}

function Write-XmachineNodeCache {
    param([Parameter(Mandatory)][hashtable]$Cache)

    $cachePath = Get-XmachineNodeCachePath
    $Cache | ConvertTo-Json -Depth 8 | Set-Content -Path $cachePath
}

function Get-CachedNodeRecord {
    param(
        [Parameter(Mandatory)][hashtable]$Cache,
        [Parameter(Mandatory)][string]$NodeId
    )

    foreach ($node in $Cache.nodes) {
        if ($node.nodeId -eq $NodeId) {
            return $node
        }
    }

    return $null
}

function Set-CachedNodeRecord {
    param(
        [Parameter(Mandatory)][hashtable]$Cache,
        [Parameter(Mandatory)][hashtable]$Record
    )

    $updated = @()
    $replaced = $false
    foreach ($node in $Cache.nodes) {
        if ($node.nodeId -eq $Record.nodeId) {
            $updated += $Record
            $replaced = $true
            continue
        }

        $updated += $node
    }

    if (-not $replaced) {
        $updated += $Record
    }

    $Cache.nodes = @($updated)
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

function New-PowerShellEncodedCommand {
    param([Parameter(Mandatory)][string]$Script)

    return [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($Script))
}

function ConvertTo-BashSingleQuotedString {
    param([Parameter(Mandatory)][string]$Value)

    return "'" + ($Value -replace "'", "'\\''") + "'"
}

function ConvertTo-PosixDoubleQuotedString {
    param([Parameter(Mandatory)][string]$Value)

    $escaped = $Value -replace '\\', '\\\\'
    $escaped = $escaped -replace '"', '\\"'
    $escaped = $escaped -replace '\$', '\\\$'
    $escaped = $escaped -replace '`', '\`'
    return $escaped
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

function Invoke-WindowsCommand {
    param(
        [Parameter(Mandatory)][string]$NodeId,
        [Parameter(Mandatory)][string]$Script
    )

    $encoded = New-PowerShellEncodedCommand -Script $Script
    return Invoke-SshCommand -NodeId $NodeId -RemoteCommand ("pwsh -NoProfile -EncodedCommand {0}" -f $encoded)
}

function Write-Stage {
    param(
        [Parameter(Mandatory)][string]$Stage,
        [Parameter(Mandatory)][string]$Detail,
        [ValidateSet("OK", "RUN", "FAIL", "INFO")]
        [string]$Status = "RUN"
    )

    Write-Host ("[{0}] control node={1} work node={2} stage={3} :: {4}" -f $Status, $env:COMPUTERNAME, $WorkNode, $Stage, $Detail)
}

function Resolve-WorkRepoPath {
    param(
        [AllowEmptyString()][string]$RequestedPath,
        [Parameter(Mandatory)][hashtable]$Cache,
        [Parameter(Mandatory)][string]$NodeId,
        [Parameter(Mandatory)][string]$RepoName,
        [string]$ConfiguredPath,
        [Parameter(Mandatory)][string]$ConfigPath
    )

    if (-not [string]::IsNullOrWhiteSpace($RequestedPath)) {
        return $RequestedPath
    }

    if (-not [string]::IsNullOrWhiteSpace($ConfiguredPath)) {
        return $ConfiguredPath
    }

    $cached = Get-CachedNodeRecord -Cache $Cache -NodeId $NodeId
    if ($null -ne $cached -and $null -ne $cached.repoPaths -and $cached.repoPaths.ContainsKey($RepoName)) {
        return $cached.repoPaths[$RepoName]
    }

    throw "No repo path is configured for work node '$NodeId' and repo '$RepoName'. Pass -WorkRepoPath once, define 'repoPath' for '$NodeId' in '$ConfigPath', or populate the cache under ~/.gal/xmachine-nodes.json."
}

function Resolve-WorkPlatform {
    param(
        [Parameter(Mandatory)][string]$Platform,
        [Parameter(Mandatory)][string]$NodeId
    )

    if ($Platform -ne "auto") {
        return $Platform
    }

    $windowsProbe = Invoke-SshCommand -NodeId $NodeId -RemoteCommand "cmd /c ver"
    if ($windowsProbe.ExitCode -eq 0) {
        return "windows"
    }

    return "posix"
}

function Assert-StageSuccess {
    param(
        [Parameter(Mandatory)][string]$Stage,
        [Parameter(Mandatory)][hashtable]$Result,
        [Parameter(Mandatory)][string]$FailureMessage
    )

    if ($Result.ExitCode -ne 0) {
        $detail = ($Result.Output -join [Environment]::NewLine).Trim()
        $detailSuffix = ""
        if ($detail) {
            $detailSuffix = "`n$detail"
        }

        Write-Stage -Stage $Stage -Detail ($FailureMessage + $detailSuffix) -Status FAIL
        throw "$Stage failed. $FailureMessage"
    }
}

function Get-CommandVersionResult {
    param(
        [Parameter(Mandatory)][string]$Platform,
        [Parameter(Mandatory)][string]$NodeId,
        [Parameter(Mandatory)][string]$CommandName,
        [Parameter(Mandatory)][string]$VersionCommand
    )

    if ($Platform -eq "windows") {
        return Invoke-WindowsCommand -NodeId $NodeId -Script $VersionCommand
    }

    return Invoke-PosixCommand -NodeId $NodeId -Script $VersionCommand
}

function Test-ToolVersion {
    param(
        [Parameter(Mandatory)][string]$Platform,
        [Parameter(Mandatory)][string]$NodeId,
        [Parameter(Mandatory)][string]$Stage,
        [Parameter(Mandatory)][string]$CommandName,
        [Parameter(Mandatory)][string]$VersionCommand
    )

    Write-Stage -Stage $Stage -Detail ("checking {0}" -f $CommandName)
    $result = Get-CommandVersionResult -Platform $Platform -NodeId $NodeId -CommandName $CommandName -VersionCommand $VersionCommand
    Assert-StageSuccess -Stage $Stage -Result $result -FailureMessage ("Failed to start {0} on work node '{1}'." -f $CommandName, $NodeId)
    $version = ($result.Output | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -First 1).Trim()
    Write-Stage -Stage $Stage -Detail ("{0} => {1}" -f $CommandName, $version) -Status OK
    return $version
}

function Test-ToolBatch {
    param(
        [Parameter(Mandatory)][string]$Platform,
        [Parameter(Mandatory)][string]$NodeId,
        [Parameter(Mandatory)][string]$Stage,
        [Parameter(Mandatory)][string]$BatchLabel,
        [Parameter(Mandatory)][object[]]$ToolSpecs
    )

    Write-Stage -Stage $Stage -Detail ("checking {0}" -f $BatchLabel)
    $details = [System.Collections.Generic.List[string]]::new()

    foreach ($tool in $ToolSpecs) {
        $result = Get-CommandVersionResult -Platform $Platform -NodeId $NodeId -CommandName $tool.CommandName -VersionCommand $tool.VersionCommand
        Assert-StageSuccess -Stage $Stage -Result $result -FailureMessage ("Failed to start {0} while checking {1} on work node '{2}'." -f $tool.CommandName, $BatchLabel, $NodeId)
        $detail = ($result.Output | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -First 1).Trim()
        if (-not $detail) {
            $detail = $tool.CommandName
        }

        $details.Add(("{0} => {1}" -f $tool.CommandName, $detail))
    }

    Write-Stage -Stage $Stage -Detail ($details -join "; ") -Status OK
    return @($details)
}

function Test-PosixDetachedLauncher {
    param(
        [Parameter(Mandatory)][string]$NodeId,
        [string]$Stage = "tool-launcher"
    )

    Write-Stage -Stage $Stage -Detail "checking detached launcher (zellij+script or nohup)"
    $result = Invoke-PosixCommand -NodeId $NodeId -Script @"
if command -v zellij >/dev/null 2>&1 && command -v script >/dev/null 2>&1; then
  printf 'zellij\t%s\n' "$(zellij --version 2>/dev/null | head -n 1)"
  exit 0
fi

if command -v nohup >/dev/null 2>&1; then
  printf 'nohup\t%s\n' "$(command -v nohup)"
  exit 0
fi

exit 1
"@
    Assert-StageSuccess -Stage $Stage -Result $result -FailureMessage ("No supported detached launcher is available on work node '{0}'. Install zellij plus script, or ensure nohup is available." -f $NodeId)

    $detail = ($result.Output | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -First 1).Trim()
    $parts = $detail -split "`t", 2
    $launcherName = if ($parts.Count -gt 0) { $parts[0] } else { $detail }
    $launcherDetail = if ($parts.Count -gt 1) { $parts[1] } else { $detail }
    Write-Stage -Stage $Stage -Detail ("{0} => {1}" -f $launcherName, $launcherDetail) -Status OK

    return @{
        Name = $launcherName
        Detail = $launcherDetail
    }
}

function Test-PipelineSmoke {
    param(
        [Parameter(Mandatory)][string]$Platform,
        [Parameter(Mandatory)][string]$NodeId,
        [Parameter(Mandatory)][string]$RepoPath,
        [string]$Stage = "pipeline-smoke"
    )

    Write-Stage -Stage $Stage -Detail "checking GAL pipeline dispatch entrypoint"

    if ($Platform -eq "windows") {
        $result = Invoke-WindowsCommand -NodeId $NodeId -Script @"
Set-Location -LiteralPath '$RepoPath'
& .\scripts\gal.ps1 dispatch pipeline smoke-test
"@
    }
    else {
        $result = Invoke-PosixCommand -NodeId $NodeId -Script ("cd `"{0}`" && bash ./scripts/gal.sh dispatch pipeline smoke-test" -f $RepoPath)
    }

    Assert-StageSuccess -Stage $Stage -Result $result -FailureMessage ("GAL pipeline dispatch smoke failed on work node '{0}'." -f $NodeId)

    $dispatchFound = [bool]($result.Output | Select-String '^COMMAND:\s+pipeline$' | Select-Object -First 1)
    if (-not $dispatchFound) {
        Write-Stage -Stage $Stage -Detail "GAL pipeline smoke completed without the expected dispatch marker." -Status FAIL
        throw "GAL pipeline dispatch marker was missing from the smoke output."
    }

    Write-Stage -Stage $Stage -Detail "GAL pipeline dispatch entrypoint is ready" -Status OK
    return @($result.Output)
}

function Write-NodeReadinessRecord {
    param(
        [Parameter(Mandatory)][hashtable]$Cache,
        [Parameter(Mandatory)][string]$NodeId,
        [Parameter(Mandatory)][string]$SshTarget,
        [Parameter(Mandatory)][string]$Platform,
        [Parameter(Mandatory)][string]$Status,
        [Parameter(Mandatory)][string[]]$VerifiedStages,
        [Parameter(Mandatory)][string]$RepoName,
        [string]$RepoPath,
        [string]$RuntimeRepoPath,
        [Parameter(Mandatory)][hashtable]$ToolRecord
    )

    $record = @{
        nodeId = $NodeId
        sshTarget = $SshTarget
        platform = $Platform
        status = $Status
        lastVerifiedAt = (Get-Date).ToString("o")
        verifiedStages = $VerifiedStages
        repoPaths = @{}
        runtimeRepoPath = $RuntimeRepoPath
        tools = $ToolRecord
    }

    if (-not [string]::IsNullOrWhiteSpace($RepoPath)) {
        $record.repoPaths[$RepoName] = $RepoPath
    }

    $existing = Get-CachedNodeRecord -Cache $Cache -NodeId $NodeId
    if ($null -ne $existing -and $null -ne $existing.repoPaths) {
        foreach ($key in $existing.repoPaths.Keys) {
            if (-not $record.repoPaths.ContainsKey($key)) {
                $record.repoPaths[$key] = $existing.repoPaths[$key]
            }
        }
    }

    Set-CachedNodeRecord -Cache $Cache -Record $record
    Write-XmachineNodeCache -Cache $Cache
    return $record
}

function New-StagedTaskSpec {
    param(
        [Parameter(Mandatory)][string]$SourceTemplate,
        [Parameter(Mandatory)][string]$Label
    )

    $tempRoot = Join-Path $env:TEMP "gal-xmachine-tests"
    $taskDir = Join-Path $tempRoot ([guid]::NewGuid().ToString())
    New-Item -ItemType Directory -Force -Path $taskDir | Out-Null
    $stagedPath = Join-Path $taskDir ("task-{0}-smoke.md" -f $Label)
    Copy-Item $SourceTemplate $stagedPath -Force
    return $stagedPath
}

function New-GeneratedTaskSpec {
    param(
        [Parameter(Mandatory)][string]$Label,
        [Parameter(Mandatory)][string]$Content
    )

    $tempRoot = Join-Path $env:TEMP "gal-xmachine-tests"
    $taskDir = Join-Path $tempRoot ([guid]::NewGuid().ToString())
    New-Item -ItemType Directory -Force -Path $taskDir | Out-Null
    $stagedPath = Join-Path $taskDir ("task-{0}-execute-smoke.md" -f $Label)
    Set-Content -Path $stagedPath -Value $Content -Encoding UTF8
    return $stagedPath
}

function New-ExecuteSmokeTaskSpec {
    param(
        [Parameter(Mandatory)][string]$Label,
        [Parameter(Mandatory)][string]$Platform,
        [Parameter(Mandatory)][string]$RemoteRuntimeRepoPath
    )

    $laneDescription = if ($Platform -eq "windows") { "remote Windows execute-mode" } else { "POSIX execute-mode" }
    $runtimeWrapper = if ($Platform -eq "windows") { "$RemoteRuntimeRepoPath\scripts\Invoke-XmachineRemoteTask.ps1" } else { "$RemoteRuntimeRepoPath/scripts/Invoke-XmachineLocalTask.sh" }
    $runtimeStartScript = if ($Platform -eq "windows") { "$RemoteRuntimeRepoPath\scripts\Start-xMachine.ps1" } else { "$RemoteRuntimeRepoPath/scripts/Start-xMachine.sh" }
    $content = @"
# Task Spec: xmachine Execute-Mode Smoke Test

## Goal

Produce a read-only smoke-test summary proving that the $laneDescription lane can read the GAL runtime checkout at `$RemoteRuntimeRepoPath` and emit the standard xmachine runtime outputs without relying on a persistent target repo checkout.

## Task Type

repo-scan

## Endpoint Class

execute-mode

## Context

This is a bounded smoke test for execute mode only. Read these runtime-checkout files:

- `$RemoteRuntimeRepoPath/.dev/project.md`
- `$RemoteRuntimeRepoPath/docs/collaborative-tools/xmachine.md`
- `$runtimeWrapper`
- `$runtimeStartScript`

## Constraints

- This task is read-only.
- Do not modify any tracked file under `$RemoteRuntimeRepoPath`.
- Do not create commits.
- Do not manually create `summary.md`, `status.json`, `runtime.log`, or `result.patch`.

## Output Format

### Final assistant response

End with a concise final assistant response that states:

- whether the required files were read successfully
- that the execute-mode lane ran without a persistent target repo checkout
- what this smoke test validated and what it did not validate
- any blockers or anomalies discovered

### result.patch

- The xmachine wrapper should produce an empty `result.patch`.

## Notes

Keep the summary concise and factual. Do not propose architecture changes in this smoke test.
"@

    return New-GeneratedTaskSpec -Label $Label -Content $content
}

try {
    $repoRoot = Get-RepoRoot
    $repoContextRoot = Get-RepoContextRoot
    $repoName = Get-RepoName -RepoRoot $repoContextRoot
    $xmachineConfig = Read-XmachineConfig -RepoRoot $repoRoot

    if ([string]::IsNullOrWhiteSpace($WorkNode)) {
        throw "WorkNode is required. Pass -WorkNode <configured-work-node-alias> and define that alias under 'xmachineNodeAliases' or legacy 'nodes' in '$($xmachineConfig.path)'."
    }

    $workNodeRecord = Get-XmachineNodeRecord -RequestedNode $WorkNode -Nodes $xmachineConfig.nodes -ConfigPath $xmachineConfig.path
    $resolvedWorkNodeTarget = Resolve-WorkNodeTarget -RequestedNode $WorkNode -NodeRecord $workNodeRecord -ConfigPath $xmachineConfig.path
    $configuredWorkRepoPath = Get-ConfiguredWorkRepoPath -NodeRecord $workNodeRecord
    $configuredRuntimeRepoPath = Get-ConfiguredRuntimeRepoPath -NodeRecord $workNodeRecord
    $repoMappingKey = Get-RepoMappingKey -RepoContextRoot $repoContextRoot
    $repoMapping = Get-ConfiguredRepoMapping -NodeRecord $workNodeRecord -RepoKey $repoMappingKey

    $remoteTemplate = Join-Path $repoRoot "templates\task-xmachine-remote-smoke.md"
    $localTemplate = Join-Path $repoRoot "templates\task-xmachine-local-smoke.md"
    $directDispatch = Join-Path $repoRoot "scripts\Invoke-XmachineTask.ps1"

    $cache = Read-XmachineNodeCache
    $cachedRecord = Get-CachedNodeRecord -Cache $cache -NodeId $WorkNode
    $cachedRepoPath = if ($null -ne $cachedRecord -and $null -ne $cachedRecord.repoPaths -and $cachedRecord.repoPaths.ContainsKey($repoName)) { [string]$cachedRecord.repoPaths[$repoName] } else { $null }
    $mappedProjectRepoPath = if ($null -ne $repoMapping -and $repoMapping.Contains("repoPath")) { [string]$repoMapping["repoPath"] } else { $null }
    $mappedRuntimeRepoPath = if ($null -ne $repoMapping -and $repoMapping.Contains("runtimeRepoPath")) { [string]$repoMapping["runtimeRepoPath"] } else { $null }
    $resolvedRepoPath = if (-not [string]::IsNullOrWhiteSpace($WorkRepoPath)) { $WorkRepoPath } elseif (-not [string]::IsNullOrWhiteSpace($mappedProjectRepoPath)) { $mappedProjectRepoPath.Trim() } elseif (-not [string]::IsNullOrWhiteSpace($configuredWorkRepoPath)) { $configuredWorkRepoPath } elseif (-not [string]::IsNullOrWhiteSpace($cachedRepoPath)) { $cachedRepoPath.Trim() } else { $null }
    $resolvedRuntimeRepoPath = if (-not [string]::IsNullOrWhiteSpace($mappedRuntimeRepoPath)) { $mappedRuntimeRepoPath.Trim() } elseif (-not [string]::IsNullOrWhiteSpace($configuredRuntimeRepoPath)) { $configuredRuntimeRepoPath } elseif (-not [string]::IsNullOrWhiteSpace($configuredWorkRepoPath)) { $configuredWorkRepoPath } elseif (-not [string]::IsNullOrWhiteSpace($resolvedRepoPath)) { $resolvedRepoPath } else { $null }
    if ([string]::IsNullOrWhiteSpace($resolvedRuntimeRepoPath)) {
        throw "No GAL runtime path is configured for work node '$WorkNode'. Define 'runtimeRepoPath', repoMappings.<repo>.runtimeRepoPath, or fall back to node-level 'repoPath' in '$($xmachineConfig.path)'."
    }

    $executionMode = if ([string]::IsNullOrWhiteSpace($resolvedRepoPath)) { "execute" } else { "repo" }
    $smokeRepoPath = if ($executionMode -eq "repo") { $resolvedRepoPath } else { $resolvedRuntimeRepoPath }
    $verifiedStages = @()
    $toolRecord = @{}

    Write-Stage -Stage "ssh-config" -Detail "checking SSH node id in .ssh/config"
    $sshConfigOutput = & ssh -G $resolvedWorkNodeTarget 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Stage -Stage "ssh-config" -Detail ((@($sshConfigOutput) -join [Environment]::NewLine).Trim()) -Status FAIL
        throw "Could not resolve SSH target '$resolvedWorkNodeTarget' for work node '$WorkNode'. Ensure the target in '$($xmachineConfig.path)' or the SSH config entry is correct."
    }

    $sshConfig = ConvertFrom-SshConfigOutput -Lines @($sshConfigOutput | ForEach-Object { $_.ToString() })
    Write-Stage -Stage "ssh-config" -Detail ("resolved work node '{0}' to SSH target '{1}' via xmachine config/.ssh config" -f $WorkNode, $resolvedWorkNodeTarget) -Status OK

    Write-Stage -Stage "ssh-batch" -Detail "checking passwordless BatchMode SSH"
    $sshBatch = Invoke-SshCommand -NodeId $resolvedWorkNodeTarget -RemoteCommand "echo XMACHINE_SSH_OK"
    Assert-StageSuccess -Stage "ssh-batch" -Result $sshBatch -FailureMessage ("Passwordless SSH failed for work node '{0}'. Configure key-based login first." -f $WorkNode)
    Write-Stage -Stage "ssh-batch" -Detail "passwordless SSH is ready" -Status OK

    $resolvedPlatform = Resolve-WorkPlatform -Platform $WorkPlatform -NodeId $resolvedWorkNodeTarget
    Write-Stage -Stage "platform" -Detail ("detected platform '{0}'" -f $resolvedPlatform) -Status OK

    if ($resolvedPlatform -eq "windows") {
        Write-Stage -Stage "repo" -Detail ("checking runtime path '{0}'" -f $smokeRepoPath)
        $repoCheck = Invoke-WindowsCommand -NodeId $resolvedWorkNodeTarget -Script @"
if (Test-Path '$smokeRepoPath\.git') {
    'XMACHINE_REPO_OK'
    exit 0
}

Write-Error 'Repo path is missing or is not a git checkout.'
exit 1
"@
        Assert-StageSuccess -Stage "repo" -Result $repoCheck -FailureMessage ("Runtime path '{0}' is not usable on work node '{1}'." -f $smokeRepoPath, $WorkNode)
        Write-Stage -Stage "repo" -Detail (if ($executionMode -eq "repo") { "project repo path is ready" } else { "runtime repo path is ready for execute-mode smoke" }) -Status OK

        $aiToolDetails = Test-ToolBatch -Platform $resolvedPlatform -NodeId $resolvedWorkNodeTarget -Stage "tool-batch-ai" -BatchLabel "AI CLIs used by the current smoke test (gemini, copilot)" -ToolSpecs @(
            @{ CommandName = "gemini"; VersionCommand = "gemini -v" },
            @{ CommandName = "copilot"; VersionCommand = "copilot -v" }
        )

        if (-not $sshConfig.ContainsKey("user")) {
            throw "SSH config for work node '$WorkNode' does not expose a User value. Add User to the Host entry in .ssh/config before running the Windows work-node smoke test."
        }

        if (-not (Test-Path $directDispatch)) {
            throw "Missing direct xmachine dispatcher: $directDispatch"
        }

        $stagedTaskSpec = if ($executionMode -eq "repo") {
            if (-not (Test-Path $remoteTemplate)) {
                throw "Missing remote smoke template: $remoteTemplate"
            }

            New-StagedTaskSpec -SourceTemplate $remoteTemplate -Label $WorkNode
        }
        else {
            New-ExecuteSmokeTaskSpec -Label $WorkNode -Platform $resolvedPlatform -RemoteRuntimeRepoPath $resolvedRuntimeRepoPath
        }

        Write-Stage -Stage "work-node-smoke" -Detail ("dispatching Windows work-node smoke via {0} ({1} mode)" -f $stagedTaskSpec, $executionMode)
        $dispatchArgs = @{
            WorkNode = $WorkNode
            TaskSpec = $stagedTaskSpec
            RemoteRuntimeRepoPath = $resolvedRuntimeRepoPath
            WorkPlatform = $resolvedPlatform
            TimeoutMinutes = $TimeoutMinutes
        }
        if ($Wait) {
            $dispatchArgs.Wait = $true
        }
        if (-not [string]::IsNullOrWhiteSpace($resolvedRepoPath)) {
            $dispatchArgs.WorkRepoPath = $resolvedRepoPath
        }
        if ($LocalOutputDir) {
            $dispatchArgs.LocalOutputDir = $LocalOutputDir
        }

        & $directDispatch @dispatchArgs

        Write-Stage -Stage "work-node-smoke" -Detail "Windows work-node smoke dispatched successfully" -Status OK
        $verifiedStages = @("ssh-config", "ssh-batch", "platform", "repo", "tool-batch-ai", "work-node-smoke")
        $toolRecord = @{
            gemini = $aiToolDetails[0]
            copilot = $aiToolDetails[1]
        }
    }
    else {
        Write-Stage -Stage "repo" -Detail ("checking runtime path '{0}'" -f $smokeRepoPath)
        $repoCheck = Invoke-PosixCommand -NodeId $resolvedWorkNodeTarget -Script ("test -d `"{0}/.git`" && echo XMACHINE_REPO_OK" -f $smokeRepoPath)
        Assert-StageSuccess -Stage "repo" -Result $repoCheck -FailureMessage ("Runtime path '{0}' is not usable on work node '{1}'." -f $smokeRepoPath, $WorkNode)
        Write-Stage -Stage "repo" -Detail (if ($executionMode -eq "repo") { "project repo path is ready" } else { "runtime repo path is ready for execute-mode smoke" }) -Status OK

        $runtimeToolSpecs = @(
            @{ CommandName = "jq"; VersionCommand = "jq --version" }
        )
        if (-not [string]::IsNullOrWhiteSpace($resolvedRepoPath)) {
            $runtimeToolSpecs = @(
                @{ CommandName = "git"; VersionCommand = "git --version" },
                @{ CommandName = "jq"; VersionCommand = "jq --version" }
            )
        }

        $coreRuntimeDetails = Test-ToolBatch -Platform $resolvedPlatform -NodeId $resolvedWorkNodeTarget -Stage "tool-batch-runtime" -BatchLabel "POSIX runtime tools" -ToolSpecs $runtimeToolSpecs
        $launcherRecord = Test-PosixDetachedLauncher -NodeId $resolvedWorkNodeTarget -Stage "tool-launcher"
        $aiToolDetails = Test-ToolBatch -Platform $resolvedPlatform -NodeId $resolvedWorkNodeTarget -Stage "tool-batch-ai" -BatchLabel "AI CLIs used by the current smoke test (gemini, copilot)" -ToolSpecs @(
            @{ CommandName = "gemini"; VersionCommand = "gemini --version || gemini -v" },
            @{ CommandName = "copilot"; VersionCommand = "copilot -v" }
        )

        $stagedTaskSpec = if ($executionMode -eq "repo") {
            if (-not (Test-Path $localTemplate)) {
                throw "Missing local smoke template: $localTemplate"
            }

            New-StagedTaskSpec -SourceTemplate $localTemplate -Label $WorkNode
        }
        else {
            New-ExecuteSmokeTaskSpec -Label $WorkNode -Platform $resolvedPlatform -RemoteRuntimeRepoPath $resolvedRuntimeRepoPath
        }

        Write-Stage -Stage "work-node-smoke" -Detail ("dispatching POSIX work-node smoke via {0} ({1} mode)" -f $stagedTaskSpec, $executionMode)
        $dispatchArgs = @{
            WorkNode = $WorkNode
            TaskSpec = $stagedTaskSpec
            RemoteRuntimeRepoPath = $resolvedRuntimeRepoPath
            WorkPlatform = $resolvedPlatform
            TimeoutMinutes = $TimeoutMinutes
        }
        if ($Wait) {
            $dispatchArgs.Wait = $true
        }
        if (-not [string]::IsNullOrWhiteSpace($resolvedRepoPath)) {
            $dispatchArgs.WorkRepoPath = $resolvedRepoPath
        }
        if ($LocalOutputDir) {
            $dispatchArgs.LocalOutputDir = $LocalOutputDir
        }

        & $directDispatch @dispatchArgs
        Write-Stage -Stage "work-node-smoke" -Detail "POSIX work-node smoke completed" -Status OK
        $verifiedStages = @("ssh-config", "ssh-batch", "platform", "repo", "tool-batch-runtime", "tool-launcher", "tool-batch-ai", "work-node-smoke")
        $toolRecord = @{
            launcher = $launcherRecord.Name
            detachedLauncher = $launcherRecord.Detail
            jq = if ($runtimeToolSpecs.Count -eq 1) { $coreRuntimeDetails[0] } else { $coreRuntimeDetails[1] }
            gemini = $aiToolDetails[0]
            copilot = $aiToolDetails[1]
        }
        if ($runtimeToolSpecs.Count -gt 1) {
            $toolRecord.git = $coreRuntimeDetails[0]
        }
    }

    $null = Write-NodeReadinessRecord -Cache $cache -NodeId $WorkNode -SshTarget $resolvedWorkNodeTarget -Platform $resolvedPlatform -Status "tooling-ready" -VerifiedStages $verifiedStages -RepoName $repoName -RepoPath $resolvedRepoPath -RuntimeRepoPath $resolvedRuntimeRepoPath -ToolRecord $toolRecord

    Write-Stage -Stage "cache" -Detail ("updated {0}" -f (Get-XmachineNodeCachePath)) -Status OK
    Write-Stage -Stage "summary" -Detail "work node passed SSH, runtime validation, tool-batch, and work-node smoke stages; cache marked it tooling-ready while GAL pipeline smoke is validated." -Status INFO

    [void](Test-PipelineSmoke -Platform $resolvedPlatform -NodeId $resolvedWorkNodeTarget -RepoPath $smokeRepoPath)
    $verifiedStages = @($verifiedStages + @("pipeline-smoke"))
    $null = Write-NodeReadinessRecord -Cache $cache -NodeId $WorkNode -SshTarget $resolvedWorkNodeTarget -Platform $resolvedPlatform -Status "readied" -VerifiedStages $verifiedStages -RepoName $repoName -RepoPath $resolvedRepoPath -RuntimeRepoPath $resolvedRuntimeRepoPath -ToolRecord $toolRecord
    Write-Stage -Stage "cache" -Detail ("updated {0} with readied status" -f (Get-XmachineNodeCachePath)) -Status OK
    Write-Stage -Stage "summary" -Detail "work node passed SSH, runtime validation, tool-batch, work-node smoke, and pipeline-smoke stages; node is now readied for bounded direct-task offload." -Status OK
}
catch {
    $message = $_.Exception.Message
    if ($message -notmatch '^[A-Za-z0-9-]+ failed\.') {
        $currentWorkNode = if ([string]::IsNullOrWhiteSpace($WorkNode)) { '<unset>' } else { $WorkNode }
        Write-Host ("[FAIL] control node={0} work node={1} stage=summary :: {2}" -f $env:COMPUTERNAME, $currentWorkNode, $message)
    }

    exit 1
}
