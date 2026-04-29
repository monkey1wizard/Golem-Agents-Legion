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
    `XMACHINE_WORK_NODE_ALIASES` inside `config.local.env`.

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

function Read-KeyValueEnvFile {
    param([Parameter(Mandatory)][string]$Path)

    $values = [ordered]@{}
    if (-not (Test-Path $Path)) {
        return $values
    }

    foreach ($line in Get-Content $Path -Encoding UTF8) {
        if ($line -match '^\s*#' -or $line -notmatch '=') {
            continue
        }

        $parts = $line.Split('=', 2)
        $key = $parts[0].Trim()
        $value = $parts[1].Trim()
        if ([string]::IsNullOrWhiteSpace($key) -or [string]::IsNullOrWhiteSpace($value)) {
            continue
        }

        $values[$key] = $value
    }

    return $values
}

function Get-ConfiguredValue {
    param(
        [Parameter(Mandatory)][System.Collections.IDictionary]$Values,
        [Parameter(Mandatory)][string]$Name
    )

    if ($Values.Contains($Name) -and -not [string]::IsNullOrWhiteSpace([string]$Values[$Name])) {
        return [string]$Values[$Name]
    }

    $userValue = [Environment]::GetEnvironmentVariable($Name, 'User')
    if (-not [string]::IsNullOrWhiteSpace($userValue)) {
        return $userValue
    }

    $processValue = [Environment]::GetEnvironmentVariable($Name, 'Process')
    if (-not [string]::IsNullOrWhiteSpace($processValue)) {
        return $processValue
    }

    return $null
}

function Read-WorkNodeAliasMap {
    param([Parameter(Mandatory)][System.Collections.IDictionary]$Values)

    $rawValue = Get-ConfiguredValue -Values $Values -Name "XMACHINE_WORK_NODE_ALIASES"
    $map = @{}
    if ([string]::IsNullOrWhiteSpace($rawValue)) {
        return $map
    }

    foreach ($entry in ($rawValue -split '\s*[;,]\s*')) {
        if ([string]::IsNullOrWhiteSpace($entry)) {
            continue
        }

        $parts = $entry.Split('=', 2)
        if ($parts.Count -ne 2) {
            throw "Invalid XMACHINE_WORK_NODE_ALIASES entry '$entry'. Use alias=ssh-target pairs separated by ';' or ','."
        }

        $alias = $parts[0].Trim()
        $target = $parts[1].Trim()
        if ([string]::IsNullOrWhiteSpace($alias) -or [string]::IsNullOrWhiteSpace($target)) {
            throw "Invalid XMACHINE_WORK_NODE_ALIASES entry '$entry'. Alias and ssh target must both be non-empty."
        }

        $map[$alias] = $target
    }

    return $map
}

function Resolve-WorkNodeTarget {
    param(
        [Parameter(Mandatory)][string]$RequestedNode,
        [Parameter(Mandatory)][hashtable]$AliasMap
    )

    if ($AliasMap.Count -eq 0) {
        throw "XMACHINE_WORK_NODE_ALIASES is required. Define strict alias=ssh-target pairs in config.local.env before running xmachine smoke tests."
    }

    if (-not $AliasMap.ContainsKey($RequestedNode)) {
        $availableAliases = ($AliasMap.Keys | Sort-Object) -join ", "
        throw "Unknown work node alias '$RequestedNode'. Define it in XMACHINE_WORK_NODE_ALIASES. Available aliases: $availableAliases"
    }

    return $AliasMap[$RequestedNode]
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

    return $raw | ConvertFrom-Json -AsHashtable
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
        [string]$ConfiguredPath
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

    throw "No repo path is configured for work node '$NodeId' and repo '$RepoName'. Pass -WorkRepoPath once, set XMACHINE_DEFAULT_WORK_REPO_PATH in config.local.env, or populate the cache under ~/.gal/xmachine-nodes.json."
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
        [Parameter(Mandatory)][string]$RepoPath,
        [Parameter(Mandatory)][hashtable]$ToolRecord
    )

    $record = @{
        nodeId = $NodeId
        sshTarget = $SshTarget
        platform = $Platform
        status = $Status
        lastVerifiedAt = (Get-Date).ToString("o")
        verifiedStages = $VerifiedStages
        repoPaths = @{
            $RepoName = $RepoPath
        }
        tools = $ToolRecord
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

try {
    $repoRoot = Get-RepoRoot
    $repoName = Get-RepoName -RepoRoot $repoRoot
    $localEnvPath = Join-Path $repoRoot "config.local.env"
    $localEnvValues = Read-KeyValueEnvFile -Path $localEnvPath
    $workNodeAliasMap = Read-WorkNodeAliasMap -Values $localEnvValues

    if ([string]::IsNullOrWhiteSpace($WorkNode)) {
        throw "WorkNode is required. Pass -WorkNode <configured-work-node-alias> and define that alias in XMACHINE_WORK_NODE_ALIASES inside config.local.env."
    }

    $resolvedWorkNodeTarget = Resolve-WorkNodeTarget -RequestedNode $WorkNode -AliasMap $workNodeAliasMap
    $configuredWorkRepoPath = Get-ConfiguredValue -Values $localEnvValues -Name "XMACHINE_DEFAULT_WORK_REPO_PATH"

    $remoteTemplate = Join-Path $repoRoot "templates\task-xmachine-remote-smoke.md"
    $remoteDispatch = Join-Path $repoRoot "scripts\Invoke-XmachineRemoteTask.ps1"
    $remoteRetrieve = Join-Path $repoRoot "scripts\Get-XmachineRemoteResult.ps1"

    $cache = Read-XmachineNodeCache
    $resolvedRepoPath = Resolve-WorkRepoPath -RequestedPath $WorkRepoPath -Cache $cache -NodeId $WorkNode -RepoName $repoName -ConfiguredPath $configuredWorkRepoPath
    $verifiedStages = @()
    $toolRecord = @{}

    Write-Stage -Stage "ssh-config" -Detail "checking SSH node id in .ssh/config"
    $sshConfigOutput = & ssh -G $resolvedWorkNodeTarget 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Stage -Stage "ssh-config" -Detail ((@($sshConfigOutput) -join [Environment]::NewLine).Trim()) -Status FAIL
        throw "Could not resolve SSH target '$resolvedWorkNodeTarget' for work node '$WorkNode'. Ensure the alias map or SSH config entry is correct."
    }

    $sshConfig = ConvertFrom-SshConfigOutput -Lines @($sshConfigOutput | ForEach-Object { $_.ToString() })
    Write-Stage -Stage "ssh-config" -Detail ("resolved work node '{0}' to SSH target '{1}' via config.local.env/.ssh config" -f $WorkNode, $resolvedWorkNodeTarget) -Status OK

    Write-Stage -Stage "ssh-batch" -Detail "checking passwordless BatchMode SSH"
    $sshBatch = Invoke-SshCommand -NodeId $resolvedWorkNodeTarget -RemoteCommand "echo XMACHINE_SSH_OK"
    Assert-StageSuccess -Stage "ssh-batch" -Result $sshBatch -FailureMessage ("Passwordless SSH failed for work node '{0}'. Configure key-based login first." -f $WorkNode)
    Write-Stage -Stage "ssh-batch" -Detail "passwordless SSH is ready" -Status OK

    $resolvedPlatform = Resolve-WorkPlatform -Platform $WorkPlatform -NodeId $resolvedWorkNodeTarget
    Write-Stage -Stage "platform" -Detail ("detected platform '{0}'" -f $resolvedPlatform) -Status OK

    if ($resolvedPlatform -eq "windows") {
        Write-Stage -Stage "repo" -Detail ("checking repo path '{0}'" -f $resolvedRepoPath)
        $repoCheck = Invoke-WindowsCommand -NodeId $resolvedWorkNodeTarget -Script @"
if (Test-Path '$resolvedRepoPath\.git') {
    'XMACHINE_REPO_OK'
    exit 0
}

Write-Error 'Repo path is missing or is not a git checkout.'
exit 1
"@
        Assert-StageSuccess -Stage "repo" -Result $repoCheck -FailureMessage ("Repo path '{0}' is not usable on work node '{1}'." -f $resolvedRepoPath, $WorkNode)
        Write-Stage -Stage "repo" -Detail "repo path is ready" -Status OK

        $zellijVersion = Test-ToolVersion -Platform $resolvedPlatform -NodeId $resolvedWorkNodeTarget -Stage "tool-zellij" -CommandName "zellij" -VersionCommand "zellij --version"
        $geminiVersion = Test-ToolVersion -Platform $resolvedPlatform -NodeId $resolvedWorkNodeTarget -Stage "tool-gemini" -CommandName "gemini" -VersionCommand "gemini -v"
        $copilotVersion = Test-ToolVersion -Platform $resolvedPlatform -NodeId $resolvedWorkNodeTarget -Stage "tool-copilot" -CommandName "copilot" -VersionCommand "copilot -v"

        if (-not $sshConfig.ContainsKey("user")) {
            throw "SSH config for work node '$WorkNode' does not expose a User value. Add User to the Host entry in .ssh/config before running the Windows work-node smoke test."
        }

        if (-not (Test-Path $remoteTemplate)) {
            throw "Missing remote smoke template: $remoteTemplate"
        }

        $stagedTaskSpec = New-StagedTaskSpec -SourceTemplate $remoteTemplate -Label $WorkNode
        Write-Stage -Stage "work-node-smoke" -Detail ("dispatching Windows work-node smoke via {0}" -f $stagedTaskSpec)
        $dispatchOutput = & $remoteDispatch `
            -RemoteHost $sshConfig["host"] `
            -RemoteUser $sshConfig["user"] `
            -RemoteRepoPath $resolvedRepoPath `
            -TaskSpec $stagedTaskSpec `
            -TimeoutMinutes $TimeoutMinutes 2>&1

        $dispatchOutput | ForEach-Object { Write-Host $_ }

        $taskId = ($dispatchOutput | Select-String 'Task ID:\s+(.+)$' | Select-Object -First 1).Matches.Groups[1].Value.Trim()
        $remoteOutputDir = ($dispatchOutput | Select-String 'Remote output:\s+(.+)$' | Select-Object -First 1).Matches.Groups[1].Value.Trim()
        if (-not $taskId -or -not $remoteOutputDir) {
            throw "Could not parse task metadata from Invoke-XmachineRemoteTask output."
        }

        if ($Wait) {
            Write-Stage -Stage "work-node-smoke" -Detail "waiting for remote smoke completion"
            $retrieveArgs = @{
                RemoteHost = $sshConfig["host"]
                RemoteUser = $sshConfig["user"]
                TaskId = $taskId
                RemoteOutputDir = $remoteOutputDir
                RemoteRepoPath = $resolvedRepoPath
                Wait = $true
                TimeoutMinutes = [Math]::Max($TimeoutMinutes, 60)
            }

            if ($LocalOutputDir) {
                $retrieveArgs.LocalOutputDir = $LocalOutputDir
            }

            & $remoteRetrieve @retrieveArgs
        }

        Write-Stage -Stage "work-node-smoke" -Detail "Windows work-node smoke dispatched successfully" -Status OK
        $verifiedStages = @("ssh-config", "ssh-batch", "platform", "repo", "tool-zellij", "tool-gemini", "tool-copilot", "work-node-smoke")
        $toolRecord = @{
            zellij = $zellijVersion
            gemini = $geminiVersion
            copilot = $copilotVersion
        }
    }
    else {
        Write-Stage -Stage "repo" -Detail ("checking repo path '{0}'" -f $resolvedRepoPath)
        $repoCheck = Invoke-PosixCommand -NodeId $resolvedWorkNodeTarget -Script ("test -d `"{0}/.git`" && echo XMACHINE_REPO_OK" -f $resolvedRepoPath)
        Assert-StageSuccess -Stage "repo" -Result $repoCheck -FailureMessage ("Repo path '{0}' is not usable on work node '{1}'." -f $resolvedRepoPath, $WorkNode)
        Write-Stage -Stage "repo" -Detail "repo path is ready" -Status OK

        $coreRuntimeDetails = Test-ToolBatch -Platform $resolvedPlatform -NodeId $resolvedWorkNodeTarget -Stage "tool-batch-runtime" -BatchLabel "POSIX core runtime tools (zellij, git, jq)" -ToolSpecs @(
            @{ CommandName = "zellij"; VersionCommand = "zellij --version" },
            @{ CommandName = "git"; VersionCommand = "git --version" },
            @{ CommandName = "jq"; VersionCommand = "jq --version" }
        )
        $scriptPath = Test-ToolVersion -Platform $resolvedPlatform -NodeId $resolvedWorkNodeTarget -Stage "tool-shell-pty" -CommandName "script" -VersionCommand "command -v script"
        $aiToolDetails = Test-ToolBatch -Platform $resolvedPlatform -NodeId $resolvedWorkNodeTarget -Stage "tool-batch-ai" -BatchLabel "AI CLIs used by the current smoke test (gemini, copilot)" -ToolSpecs @(
            @{ CommandName = "gemini"; VersionCommand = "gemini --version || gemini -v" },
            @{ CommandName = "copilot"; VersionCommand = "copilot -v" }
        )

        $taskSpecPath = "{0}/templates/task-xmachine-local-smoke.md" -f $resolvedRepoPath
        Write-Stage -Stage "work-node-smoke" -Detail "dispatching POSIX work-node smoke directly from the control node"
        $posixDispatch = Invoke-PosixCommand -NodeId $resolvedWorkNodeTarget -Script ("cd `"{0}`" && bash scripts/Invoke-XmachineLocalTask.sh --task-spec `"{1}`" --repo-path `"{0}`" --timeout-minutes {2}" -f $resolvedRepoPath, $taskSpecPath, $TimeoutMinutes)
        Assert-StageSuccess -Stage "work-node-smoke" -Result $posixDispatch -FailureMessage ("POSIX work-node dispatch failed for '{0}'." -f $WorkNode)
        $posixDispatch.Output | ForEach-Object { Write-Host $_ }

        $localTaskId = ($posixDispatch.Output | Select-String '^[ ]*TaskId:\s+(.+)$' | Select-Object -First 1).Matches.Groups[1].Value.Trim()
        $localOutputDir = ($posixDispatch.Output | Select-String '^[ ]*Output:\s+(.+)$' | Select-Object -First 1).Matches.Groups[1].Value.Trim()
        if (-not $localTaskId -or -not $localOutputDir) {
            throw "Could not parse TaskId or Output from the POSIX work-node dispatch output."
        }

        Write-Stage -Stage "work-node-smoke" -Detail "waiting for POSIX work-node smoke completion"
        $posixRetrieve = Invoke-PosixCommand -NodeId $resolvedWorkNodeTarget -Script ("cd `"{0}`" && bash scripts/Get-XmachineLocalResult.sh --task-id `"{1}`" --output-dir `"{2}`" --wait --timeout-minutes {3} --repo-path `"{0}`"" -f $resolvedRepoPath, $localTaskId, $localOutputDir, [Math]::Max($TimeoutMinutes, 60))
        Assert-StageSuccess -Stage "work-node-smoke" -Result $posixRetrieve -FailureMessage ("POSIX work-node smoke failed for '{0}'." -f $WorkNode)
        $posixRetrieve.Output | ForEach-Object { Write-Host $_ }
        Write-Stage -Stage "work-node-smoke" -Detail "POSIX work-node smoke completed" -Status OK
        $verifiedStages = @("ssh-config", "ssh-batch", "platform", "repo", "tool-batch-runtime", "tool-shell-pty", "tool-batch-ai", "work-node-smoke")
        $toolRecord = @{
            zellij = $coreRuntimeDetails[0]
            git = $coreRuntimeDetails[1]
            jq = $coreRuntimeDetails[2]
            script = $scriptPath
            gemini = $aiToolDetails[0]
            copilot = $aiToolDetails[1]
        }
    }

    $null = Write-NodeReadinessRecord -Cache $cache -NodeId $WorkNode -SshTarget $resolvedWorkNodeTarget -Platform $resolvedPlatform -Status "tooling-ready" -VerifiedStages $verifiedStages -RepoName $repoName -RepoPath $resolvedRepoPath -ToolRecord $toolRecord

    Write-Stage -Stage "cache" -Detail ("updated {0}" -f (Get-XmachineNodeCachePath)) -Status OK
    Write-Stage -Stage "summary" -Detail "work node passed SSH, repo, tool-batch, and work-node smoke stages; cache marked it tooling-ready while GAL pipeline smoke is validated." -Status INFO

    $pipelineSmokeOutput = Test-PipelineSmoke -Platform $resolvedPlatform -NodeId $resolvedWorkNodeTarget -RepoPath $resolvedRepoPath
    $verifiedStages = @($verifiedStages + @("pipeline-smoke"))
    $null = Write-NodeReadinessRecord -Cache $cache -NodeId $WorkNode -SshTarget $resolvedWorkNodeTarget -Platform $resolvedPlatform -Status "readied" -VerifiedStages $verifiedStages -RepoName $repoName -RepoPath $resolvedRepoPath -ToolRecord $toolRecord
    Write-Stage -Stage "cache" -Detail ("updated {0} with readied status" -f (Get-XmachineNodeCachePath)) -Status OK
    Write-Stage -Stage "summary" -Detail "work node passed SSH, repo, tool-batch, work-node smoke, and pipeline-smoke stages; node is now readied for pipeline offload." -Status OK
}
catch {
    $message = $_.Exception.Message
    if ($message -notmatch '^[A-Za-z0-9-]+ failed\.') {
        $currentWorkNode = if ([string]::IsNullOrWhiteSpace($WorkNode)) { '<unset>' } else { $WorkNode }
        Write-Host ("[FAIL] control node={0} work node={1} stage=summary :: {2}" -f $env:COMPUTERNAME, $currentWorkNode, $message)
    }

    exit 1
}