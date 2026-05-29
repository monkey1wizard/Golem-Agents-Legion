#Requires -Version 5.1
param(
    [switch]$Uninstall,
    [switch]$Replace,
    [switch]$DryRun,
    [switch]$Reconfigure,
    [string[]]$SelectedRuntimes,
    [string]$PrimaryRuntime
)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'common\Common.ps1')

function Get-GalMachineConfig {
    $context = $script:SetupContext
    if (-not (Test-Path $context.GalConfigFile)) {
        return [ordered]@{}
    }

    return Read-JsonOrderedMap $context.GalConfigFile
}

function Get-McpVariableMap([System.Collections.IDictionary]$LocalEnvValues, [System.Collections.IDictionary]$MachineConfig) {
    $values = [ordered]@{}
    foreach ($key in $LocalEnvValues.Keys) {
        $values[$key] = $LocalEnvValues[$key]
    }

    if ($MachineConfig) {
        $machineMappings = [ordered]@{
            OBSIDIAN_VAULT = 'obsidianVault'
            OBSIDIAN_VAULT_NAME = 'obsidianVaultName'
            OBSIDIAN_GUIDE_PATH = 'obsidianGuidePath'
            OBSIDIAN_GUIDE_MODE = 'obsidianGuideMode'
            CONTEXT7_API_KEY = 'context7ApiKey'
            TEMP_DIR = 'tempDir'
            LOCAL_SEARCH_PROJECT = 'localSearchProject'
        }

        foreach ($mappedName in $machineMappings.Keys) {
            $configKey = [string]$machineMappings[$mappedName]
            if ($MachineConfig.Contains($configKey) -and -not [string]::IsNullOrWhiteSpace([string]$MachineConfig[$configKey])) {
                $values[$mappedName] = [string]$MachineConfig[$configKey]
            }
        }

        if ($MachineConfig.Contains('mcpFilesystemPaths')) {
            $configuredPaths = $MachineConfig['mcpFilesystemPaths']
            if ($configuredPaths -is [System.Collections.IEnumerable] -and -not ($configuredPaths -is [string])) {
                $values['MCP_FILESYSTEM_PATHS'] = (@($configuredPaths | ForEach-Object { [string]$_ }) -join ',')
            }
            elseif (-not [string]::IsNullOrWhiteSpace([string]$configuredPaths)) {
                $values['MCP_FILESYSTEM_PATHS'] = [string]$configuredPaths
            }
        }
    }

    if (-not $values.Contains('MCP_FILESYSTEM_PATHS')) {
        $paths = @((Split-Path $script:SetupContext.RepoRoot -Parent))
        $obsidianVault = Get-ConfiguredValue $values 'OBSIDIAN_VAULT'
        if (-not [string]::IsNullOrWhiteSpace($obsidianVault)) {
            $paths += $obsidianVault
        }
        $values['MCP_FILESYSTEM_PATHS'] = (($paths | Select-Object -Unique) -join ',')
    }

    return $values
}

function Resolve-McpString([string]$Value, [System.Collections.IDictionary]$Values) {
    if ($Value -match '^\$\{([A-Z0-9_]+)\}$') {
        $resolved = Get-ConfiguredValue $Values $Matches[1]
        if ($null -ne $resolved) { return $resolved }
    }

    return [regex]::Replace($Value, '\$\{([A-Z0-9_]+)\}', {
        param($match)
        $resolved = Get-ConfiguredValue $Values $match.Groups[1].Value
        if ($null -ne $resolved) { return $resolved }
        return $match.Value
    })
}

function Resolve-McpNode([object]$Node, [System.Collections.IDictionary]$Values, [switch]$ExpandArrayPlaceholder) {
    if ($null -eq $Node) { return $null }

    if ($Node -is [string]) {
        if ($ExpandArrayPlaceholder -and $Node -match '^\$\{([A-Z0-9_]+)\[\]\}$') {
            return @(Split-ConfigList (Get-ConfiguredValue $Values $Matches[1]))
        }
        return Resolve-McpString $Node $Values
    }

    if ($Node -is [System.Collections.IDictionary]) {
        $map = [ordered]@{}
        foreach ($key in $Node.Keys) {
            $map[$key] = Resolve-McpNode $Node[$key] $Values
        }
        return $map
    }

    if ($Node -is [System.Collections.IEnumerable] -and -not ($Node -is [string])) {
        $items = [System.Collections.Generic.List[object]]::new()
        foreach ($item in $Node) {
            $resolvedItem = Resolve-McpNode $item $Values -ExpandArrayPlaceholder:$ExpandArrayPlaceholder
            if ($ExpandArrayPlaceholder -and $resolvedItem -is [System.Collections.IEnumerable] -and -not ($resolvedItem -is [string])) {
                foreach ($resolvedChild in @($resolvedItem)) {
                    $items.Add($resolvedChild)
                }
            }
            else {
                $items.Add($resolvedItem)
            }
        }
        return ,([object[]]$items.ToArray())
    }

    return $Node
}

function Resolve-McpConfig([System.Collections.IDictionary]$Config, [System.Collections.IDictionary]$Values) {
    $resolved = [ordered]@{}
    foreach ($key in $Config.Keys) {
        $resolved[$key] = Resolve-McpNode $Config[$key] $Values -ExpandArrayPlaceholder:($key -eq 'args')
    }
    return $resolved
}

function Resolve-McpInputs([object]$Inputs, [System.Collections.IDictionary]$Values) {
    if ($null -eq $Inputs -or $Inputs -is [string] -or $Inputs -isnot [System.Collections.IEnumerable]) {
        return @()
    }

    $resolved = [System.Collections.Generic.List[object]]::new()
    foreach ($input in @($Inputs)) {
        $resolved.Add((Resolve-McpNode $input $Values))
    }

    return @($resolved.ToArray())
}

function Normalize-McpServers([System.Collections.IDictionary]$Servers) {
    $normalized = [ordered]@{}
    $preferGithubAlias = $Servers.Contains('github')
    foreach ($serverName in $Servers.Keys) {
        if ($preferGithubAlias -and $serverName -eq 'github-mcp-server') {
            continue
        }

        $normalized[$serverName] = $Servers[$serverName]
    }

    return $normalized
}

function Test-McpManifestContainsFilesystemServer([System.Collections.IDictionary]$Manifest) {
    if (-not $Manifest.Contains('servers') -or $Manifest['servers'] -isnot [System.Collections.IDictionary]) {
        return $false
    }

    foreach ($serverName in $Manifest['servers'].Keys) {
        if ([string]$serverName -match 'filesystem') {
            return $true
        }

        $serverConfig = $Manifest['servers'][$serverName]
        if ($serverConfig -is [System.Collections.IDictionary]) {
            if ($serverConfig.Contains('command') -and [string]$serverConfig['command'] -match 'filesystem') {
                return $true
            }

            if ($serverConfig.Contains('args')) {
                foreach ($argument in @($serverConfig['args'])) {
                    if ([string]$argument -match 'filesystem') {
                        return $true
                    }
                }
            }
        }
    }

    return $false
}

function ConvertTo-McpProjectionServer([System.Collections.IDictionary]$ServerConfig) {
    $projection = [ordered]@{}

    if ($ServerConfig.Contains('url')) {
        $projection['serverUrl'] = [string]$ServerConfig['url']
    }
    if ($ServerConfig.Contains('command')) {
        $projection['command'] = [string]$ServerConfig['command']
    }
    if ($ServerConfig.Contains('args')) {
        $projection['args'] = @($ServerConfig['args'] | ForEach-Object { [string]$_ })
    }
    if ($ServerConfig.Contains('env') -and $ServerConfig['env'] -is [System.Collections.IDictionary]) {
        $projection['env'] = ConvertTo-OrderedMap $ServerConfig['env']
    }
    if ($ServerConfig.Contains('headers') -and $ServerConfig['headers'] -is [System.Collections.IDictionary]) {
        $projection['headers'] = ConvertTo-OrderedMap $ServerConfig['headers']
    }
    if ($ServerConfig.Contains('tools')) {
        $projection['tools'] = ConvertTo-OrderedMap $ServerConfig['tools']
    }

    return $projection
}

function New-ManagedMcpProjection([System.Collections.IDictionary]$ResolvedManifest, [System.Collections.IDictionary]$McpValues, [System.Collections.IDictionary]$RuntimeEntries) {
    $context = $script:SetupContext
    $projectionServers = [ordered]@{}
    foreach ($serverName in $ResolvedManifest['servers'].Keys) {
        $resolvedServer = $ResolvedManifest['servers'][$serverName]
        $projectionServers[$serverName] = ConvertTo-McpProjectionServer $resolvedServer
    }

    $projection = [ordered]@{
        schemaVersion = 1
        generatedAt = (Get-Date).ToString('o')
        generatedBy = 'Update-Mcp.ps1'
        mcpServers = $projectionServers
        _metadata = [ordered]@{
            ownership = 'gal-managed'
            secretBearing = ($projectionServers.Values | Where-Object {
                $_ -is [System.Collections.IDictionary] -and
                $_.Contains('headers') -and
                $_['headers'] -is [System.Collections.IDictionary] -and
                $_['headers'].Contains('CONTEXT7_API_KEY')
            } | Measure-Object).Count -gt 0
            sourceFiles = [ordered]@{
                trackedManifest = $context.McpSourceFile
                localOverrideManifest = $context.McpLocalFile
                machineConfig = $context.GalConfigFile
            }
            runtimeEntries = (ConvertTo-OrderedMap $RuntimeEntries)
            preservation = 'Runtime config updates replace only GAL-managed MCP entries and preserve unrelated user-owned entries.'
        }
    }

    if ($ResolvedManifest.Contains('inputs')) {
        $projection['inputs'] = ConvertTo-OrderedMap $ResolvedManifest['inputs']
    }

    if (Test-McpManifestContainsFilesystemServer $ResolvedManifest) {
        $projection['mcpFilesystemPaths'] = @(Split-ConfigList (Get-ConfiguredValue $McpValues 'MCP_FILESYSTEM_PATHS'))
    }

    return $projection
}

function New-DefaultXmachineBinding {
    return [ordered]@{
        schemaVersion = 1
        defaultXmachineNode = ''
        xmachineNodeAliases = [ordered]@{}
        machineProfiles = [ordered]@{}
        localPluginPaths = @()
        providerPathOverrides = [ordered]@{}
        additionalBindings = [ordered]@{}
    }
}

function Get-ResolvedLocalEnvFilePath {
    $context = $script:SetupContext
    $primaryEnvFile = Join-Path $context.GalConfigRoot 'config.local.env'
    $legacyEnvFile = Join-Path $context.RepoRoot 'config.local.env'

    if (Test-Path $primaryEnvFile) {
        return $primaryEnvFile
    }

    if (Test-Path $legacyEnvFile) {
        return $legacyEnvFile
    }

    return $primaryEnvFile
}

function Remove-EnvKeyFromFile {
    param(
        [string]$Path,
        [string]$Key
    )

    if (-not (Test-Path $Path)) {
        return $false
    }

    $pattern = '^(\s*){0}\s*=' -f [regex]::Escape($Key)
    $lines = Get-Content $Path -Encoding UTF8
    $filteredLines = [System.Collections.Generic.List[string]]::new()
    $removed = $false

    foreach ($line in $lines) {
        if ($line -match $pattern) {
            $removed = $true
            continue
        }

        $filteredLines.Add($line)
    }

    if (-not $removed) {
        return $false
    }

    if ($script:SetupOptions.DryRun) {
        Write-Host "  [DRY RUN] Would remove $Key from: $Path"
        return $true
    }

    [System.IO.File]::WriteAllLines($Path, $filteredLines, $script:SetupContext.Utf8NoBom)
    return $true
}

function Sync-LegacyGalSkillsStore {
    param([System.Collections.IDictionary]$XmachineBinding)

    $context = $script:SetupContext
    $legacyPluginRoot = Join-Path $context.GalStorePluginsRoot 'legacy-gal-skills'
    if (-not (Test-Path $legacyPluginRoot)) {
        return
    }

    $legacyPluginRootText = [string]$legacyPluginRoot
    foreach ($pluginPath in @($XmachineBinding['localPluginPaths'])) {
        if ([string]$pluginPath -eq $legacyPluginRootText) {
            return
        }
    }

    if ($script:SetupOptions.DryRun) {
        Write-Host "  [DRY RUN] Would remove stale legacy GAL_SKILLS store: $legacyPluginRoot"
        return
    }

    Remove-Item -LiteralPath $legacyPluginRoot -Recurse -Force
    Write-Host "  [CLEANUP] Removed stale legacy GAL_SKILLS store: $legacyPluginRoot"
}

function Import-LegacyGalSkillsIntoXmachineBinding {
    param([System.Collections.IDictionary]$XmachineBinding)

    $context = $script:SetupContext
    $envFile = Get-ResolvedLocalEnvFilePath
    $envValues = Read-KeyValueEnvFile $envFile
    if (-not $envValues.Contains('GAL_SKILLS')) {
        Sync-LegacyGalSkillsStore -XmachineBinding $XmachineBinding
        return $XmachineBinding
    }

    $legacyRoots = @(Split-ConfigList ([string]$envValues['GAL_SKILLS']))
    if ($legacyRoots.Count -eq 0) {
        if (Remove-EnvKeyFromFile -Path $envFile -Key 'GAL_SKILLS') {
            if (-not $script:SetupOptions.DryRun) {
                Write-Host "  [CLEANUP] Removed empty GAL_SKILLS from: $envFile"
            }
        }

        Sync-LegacyGalSkillsStore -XmachineBinding $XmachineBinding
        return $XmachineBinding
    }

    $canonicalPluginRoot = Get-GalPluginRoot -PluginId 'gal'
    $canonicalSkillsRoot = Join-Path $canonicalPluginRoot 'skills'
    $copiedAny = $false
    $canRemoveLegacyKey = $true
    $localPluginPaths = [System.Collections.Generic.List[string]]::new()

    foreach ($path in @($XmachineBinding['localPluginPaths'])) {
        if (-not [string]::IsNullOrWhiteSpace([string]$path) -and -not $localPluginPaths.Contains([string]$path)) {
            $localPluginPaths.Add([string]$path)
        }
    }

    foreach ($legacyRoot in $legacyRoots) {
        if (-not (Test-Path $legacyRoot)) {
            Write-Host "  [WARN] GAL_SKILLS path not found; preserving GAL_SKILLS for manual follow-up: $legacyRoot" -ForegroundColor Yellow
            $canRemoveLegacyKey = $false
            continue
        }

        $skillDirs = @(Get-ChildItem $legacyRoot -Directory | Sort-Object Name)
        if ($skillDirs.Count -eq 0) {
            Write-Host "  [WARN] GAL_SKILLS path contains no skill directories; preserving GAL_SKILLS for manual follow-up: $legacyRoot" -ForegroundColor Yellow
            $canRemoveLegacyKey = $false
            continue
        }

        foreach ($skillDir in $skillDirs) {
            $skillFile = Join-Path $skillDir.FullName 'SKILL.md'
            if (-not (Test-Path $skillFile)) {
                continue
            }

            $destinationDir = Join-Path $canonicalSkillsRoot $skillDir.Name
            $destinationFile = Join-Path $destinationDir 'SKILL.md'
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would import legacy GAL_SKILLS skill: $($skillDir.FullName) -> $destinationFile"
            }
            else {
                New-Item -ItemType Directory -Path $destinationDir -Force | Out-Null
                Copy-Item $skillFile $destinationFile -Force
                Write-Host "  [MIGRATE] Imported legacy GAL_SKILLS skill: $($skillDir.Name)"
            }

            $copiedAny = $true
        }
    }

    $XmachineBinding['localPluginPaths'] = @($localPluginPaths)

    if ($canRemoveLegacyKey -and (Remove-EnvKeyFromFile -Path $envFile -Key 'GAL_SKILLS')) {
        if (-not $script:SetupOptions.DryRun) {
            Write-Host "  [CLEANUP] Removed GAL_SKILLS from: $envFile"
        }
    }

    Sync-LegacyGalSkillsStore -XmachineBinding $XmachineBinding

    return $XmachineBinding
}

function Get-ResolvedXmachineBinding {
    $context = $script:SetupContext
    $binding = New-DefaultXmachineBinding
    $legacyConfigPath = Join-Path $context.RepoRoot 'xmachine.config.json'

    if (Test-Path $context.GalXmachineConfigFile) {
        $configuredBinding = Read-JsonOrderedMap $context.GalXmachineConfigFile
        if ($null -eq $configuredBinding) {
            return $null
        }

        if ($configuredBinding.Contains('nodes') -and -not $configuredBinding.Contains('xmachineNodeAliases')) {
            $configuredBinding['xmachineNodeAliases'] = ConvertTo-OrderedMap $configuredBinding['nodes']
        }

        return Import-LegacyGalSkillsIntoXmachineBinding -XmachineBinding (Merge-OrderedMap $binding $configuredBinding)
    }

    if (Test-Path $legacyConfigPath) {
        $legacyBinding = Read-JsonOrderedMap $legacyConfigPath
        if ($null -eq $legacyBinding) {
            return $null
        }

        if ($legacyBinding.Contains('nodes')) {
            $binding['xmachineNodeAliases'] = ConvertTo-OrderedMap $legacyBinding['nodes']
        }
    }

    return Import-LegacyGalSkillsIntoXmachineBinding -XmachineBinding $binding
}

function New-ManagedXmachineProjection([System.Collections.IDictionary]$XmachineBinding) {
    $context = $script:SetupContext
    $projection = [ordered]@{
        schemaVersion = 1
        generatedAt = (Get-Date).ToString('o')
        generatedBy = 'Update-Mcp.ps1'
        defaultXmachineNode = [string]$XmachineBinding['defaultXmachineNode']
        nodes = if ($XmachineBinding.Contains('xmachineNodeAliases')) { ConvertTo-OrderedMap $XmachineBinding['xmachineNodeAliases'] } else { [ordered]@{} }
        _metadata = [ordered]@{
            ownership = 'gal-managed'
            secretBearing = $false
            sourceFiles = [ordered]@{
                machineBinding = $context.GalXmachineConfigFile
                legacyRepoConfig = (Join-Path $context.RepoRoot 'xmachine.config.json')
            }
        }
    }

    foreach ($optionalKey in @('machineProfiles', 'localPluginPaths', 'providerPathOverrides', 'additionalBindings')) {
        if ($XmachineBinding.Contains($optionalKey)) {
            $projection[$optionalKey] = ConvertTo-OrderedMap $XmachineBinding[$optionalKey]
        }
    }

    return $projection
}

function Get-PreviousManagedProjection {
    $context = $script:SetupContext
    if (-not (Test-Path $context.GalGeneratedMcpFile)) {
        return [ordered]@{}
    }

    return Read-JsonOrderedMap $context.GalGeneratedMcpFile
}

function Get-ProjectionRuntimeEntries([System.Collections.IDictionary]$Projection, [string]$RuntimeName) {
    if (
        $Projection -and
        $Projection.Contains('_metadata') -and
        $Projection['_metadata'] -is [System.Collections.IDictionary] -and
        $Projection['_metadata'].Contains('runtimeEntries') -and
        $Projection['_metadata']['runtimeEntries'] -is [System.Collections.IDictionary] -and
        $Projection['_metadata']['runtimeEntries'].Contains($RuntimeName) -and
        $Projection['_metadata']['runtimeEntries'][$RuntimeName] -is [System.Collections.IDictionary]
    ) {
        return $Projection['_metadata']['runtimeEntries'][$RuntimeName]
    }

    return [ordered]@{}
}

function Test-CanReplaceManagedRuntimeEntry {
    param(
        [System.Collections.IDictionary]$PreviousRuntimeEntries,
        [string]$Key,
        [object]$ExistingConfig,
        [object]$DesiredConfig,
        [string]$RuntimeLabel
    )

    if ($null -eq $ExistingConfig) {
        return $true
    }

    if ($PreviousRuntimeEntries -and $PreviousRuntimeEntries.Contains($Key)) {
        return $true
    }

    if ($null -ne $DesiredConfig -and (Test-JsonLikeEqual $ExistingConfig $DesiredConfig)) {
        return $true
    }

    Write-Host "  [WARN] Preserving user-owned $RuntimeLabel MCP entry: $Key" -ForegroundColor Yellow
    return $false
}

function Find-CopilotCliManagedConfigForKey([System.Collections.IDictionary]$ManagedManifest, [string]$Key) {
    foreach ($serverName in $ManagedManifest['servers'].Keys) {
        $bridgeConfig = Get-CopilotCliBridgeProfile -ServerName $serverName
        $converted = ConvertTo-CopilotCliMcpConfig $ManagedManifest['servers'][$serverName]
        if ($bridgeConfig['Enabled'] -and [string]$bridgeConfig['Key'] -eq $Key) {
            return $converted
        }

        foreach ($legacyAlias in (Get-LegacyManagedMcpAliases -RuntimeName 'copilot-cli' -ServerName $serverName)) {
            if ($legacyAlias -eq $Key) {
                return $converted
            }
        }
    }

    return $null
}

function Write-GeneratedProjectionFiles([System.Collections.IDictionary]$ResolvedManifest, [System.Collections.IDictionary]$McpValues, [System.Collections.IDictionary]$RuntimeEntries) {
    $context = $script:SetupContext

    $xmachineBinding = Get-ResolvedXmachineBinding
    if ($null -eq $xmachineBinding) {
        return
    }

    if ($script:SetupOptions.DryRun) {
        Write-Host "  [DRY RUN] Would render managed MCP projection: $($context.GalGeneratedMcpFile)"
        Write-Host "  [DRY RUN] Would render managed xmachine projection: $($context.GalGeneratedXmachineFile)"
        return
    }

    if (-not (Test-Path $context.GalXmachineConfigFile)) {
        Write-JsonOrderedMap $context.GalXmachineConfigFile $xmachineBinding
        Write-Host "  [OK] Created machine xmachine binding file: $($context.GalXmachineConfigFile)"
    }

    Write-JsonOrderedMap $context.GalGeneratedMcpFile (New-ManagedMcpProjection -ResolvedManifest $ResolvedManifest -McpValues $McpValues -RuntimeEntries $RuntimeEntries)
    Write-Host "  [OK] $($context.GalGeneratedMcpFile)"

    Write-JsonOrderedMap $context.GalGeneratedXmachineFile (New-ManagedXmachineProjection -XmachineBinding $xmachineBinding)
    Write-Host "  [OK] $($context.GalGeneratedXmachineFile)"
}

function Test-JsonLikeEqual([object]$Left, [object]$Right) {
    return ((ConvertTo-OrderedMap $Left | ConvertTo-Json -Depth 20) -eq (ConvertTo-OrderedMap $Right | ConvertTo-Json -Depth 20))
}

function Sync-ManagedMcpInputs([System.Collections.IDictionary]$Data, [System.Collections.IDictionary]$ManagedManifest) {
    if (-not $ManagedManifest.Contains('inputs')) {
        return $false
    }

    $managedInputs = @($ManagedManifest['inputs'])
    $managedIds = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($input in $managedInputs) {
        if ($input -is [System.Collections.IDictionary] -and $input.Contains('id') -and -not [string]::IsNullOrWhiteSpace([string]$input['id'])) {
            [void]$managedIds.Add([string]$input['id'])
        }
    }

    $preservedInputs = [System.Collections.Generic.List[object]]::new()
    if ($Data.Contains('inputs') -and $Data['inputs'] -is [System.Collections.IEnumerable] -and -not ($Data['inputs'] -is [string])) {
        foreach ($existing in @($Data['inputs'])) {
            if ($existing -is [System.Collections.IDictionary] -and $existing.Contains('id') -and $managedIds.Contains([string]$existing['id'])) {
                continue
            }

            $preservedInputs.Add($existing)
        }
    }

    $newInputs = [System.Collections.Generic.List[object]]::new()
    foreach ($preserved in $preservedInputs) {
        $newInputs.Add($preserved)
    }
    foreach ($managed in $managedInputs) {
        $newInputs.Add($managed)
    }

    $newValue = @($newInputs.ToArray())
    $currentValue = if ($Data.Contains('inputs')) { $Data['inputs'] } else { @() }
    if (-not (Test-JsonLikeEqual $currentValue $newValue)) {
        $Data['inputs'] = $newValue
        return $true
    }

    return $false
}

function Remove-ManagedMcpInputs([System.Collections.IDictionary]$Data, [System.Collections.IDictionary]$ManagedManifest) {
    if (-not $Data.Contains('inputs') -or $Data['inputs'] -isnot [System.Collections.IEnumerable] -or $Data['inputs'] -is [string]) {
        return $false
    }

    if (-not $ManagedManifest.Contains('inputs')) {
        return $false
    }

    $managedIds = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($managedInput in @($ManagedManifest['inputs'])) {
        if ($managedInput -is [System.Collections.IDictionary] -and $managedInput.Contains('id') -and -not [string]::IsNullOrWhiteSpace([string]$managedInput['id'])) {
            [void]$managedIds.Add([string]$managedInput['id'])
        }
    }

    if ($managedIds.Count -eq 0) {
        return $false
    }

    $preservedInputs = [System.Collections.Generic.List[object]]::new()
    foreach ($existingInput in @($Data['inputs'])) {
        if ($existingInput -is [System.Collections.IDictionary] -and $existingInput.Contains('id') -and $managedIds.Contains([string]$existingInput['id'])) {
            continue
        }

        $preservedInputs.Add($existingInput)
    }

    $newInputs = @($preservedInputs.ToArray())
    if (Test-JsonLikeEqual $Data['inputs'] $newInputs) {
        return $false
    }

    if ($newInputs.Count -eq 0) {
        $Data.Remove('inputs')
    }
    else {
        $Data['inputs'] = $newInputs
    }

    return $true
}

function ConvertTo-AgyMcpConfig([System.Collections.IDictionary]$Config) {
    $converted = [ordered]@{}

    foreach ($key in $Config.Keys) {
        if ($key -eq 'type') { continue }
        if ($key -eq 'url') {
            $converted['serverUrl'] = [string]$Config['url']
            continue
        }

        $converted[$key] = $Config[$key]
    }

    return $converted
}

function ConvertTo-CodexMcpConfig([System.Collections.IDictionary]$Config) {
    $converted = [ordered]@{}
    foreach ($key in $Config.Keys) {
        if ($key -eq 'type') { continue }
        $converted[$key] = $Config[$key]
    }
    return $converted
}

function Get-CodexBridgeProfile([string]$ServerName) {
    switch ($ServerName) {
        'chromedevtools/chrome-devtools-mcp' { return [ordered]@{ Enabled = $true; Key = 'chrome-devtools' } }
        'github-mcp-server' { return [ordered]@{ Enabled = $false; Key = $null } }
        'microsoftdocs/mcp' { return [ordered]@{ Enabled = $true; Key = 'microsoftdocs' } }
        'microsoft/markitdown' { return [ordered]@{ Enabled = $true; Key = 'markitdown' } }
        'playwright' { return [ordered]@{ Enabled = $true; Key = 'playwright' } }
        'upstash/context7' { return [ordered]@{ Enabled = $true; Key = 'context7' } }
        'imageFetch' { return [ordered]@{ Enabled = $true; Key = 'imageFetch' } }
        'blender' { return [ordered]@{ Enabled = $true; Key = 'blender' } }
        'freecad' { return [ordered]@{ Enabled = $true; Key = 'freecad' } }
        default {
            $normalized = $ServerName -replace '^[^A-Za-z0-9]+', '' -replace '[^A-Za-z0-9_-]+', '-'
            if ([string]::IsNullOrWhiteSpace($normalized)) {
                $normalized = 'server'
            }
            return [ordered]@{ Enabled = $true; Key = $normalized }
        }
    }
}

function Get-CopilotCliBridgeProfile([string]$ServerName) {
    switch ($ServerName) {
        'chromedevtools/chrome-devtools-mcp' { return [ordered]@{ Enabled = $true; Key = 'chrome-devtools' } }
        'github-mcp-server' { return [ordered]@{ Enabled = $false; Key = $null } }
        'microsoftdocs/mcp' { return [ordered]@{ Enabled = $true; Key = 'microsoftdocs' } }
        'microsoft/markitdown' { return [ordered]@{ Enabled = $true; Key = 'markitdown' } }
        'playwright' { return [ordered]@{ Enabled = $true; Key = 'playwright' } }
        'upstash/context7' { return [ordered]@{ Enabled = $true; Key = 'context7' } }
        'blender' { return [ordered]@{ Enabled = $true; Key = 'blender' } }
        'freecad' { return [ordered]@{ Enabled = $true; Key = 'freecad' } }
        default {
            $normalized = $ServerName -replace '^[^A-Za-z0-9]+', '' -replace '[^A-Za-z0-9_-]+', '-'
            if ([string]::IsNullOrWhiteSpace($normalized)) {
                $normalized = 'server'
            }
            return [ordered]@{ Enabled = $true; Key = $normalized.ToLowerInvariant() }
        }
    }
}

function Get-ClaudeBridgeProfile([string]$ServerName) {
    switch ($ServerName) {
        'chromedevtools/chrome-devtools-mcp' { return [ordered]@{ Enabled = $true; Key = 'chrome-devtools' } }
        'github-mcp-server' { return [ordered]@{ Enabled = $false; Key = $null } }
        'microsoftdocs/mcp' { return [ordered]@{ Enabled = $true; Key = 'microsoftdocs' } }
        'microsoft/markitdown' { return [ordered]@{ Enabled = $true; Key = 'markitdown' } }
        'playwright' { return [ordered]@{ Enabled = $true; Key = 'playwright' } }
        'upstash/context7' { return [ordered]@{ Enabled = $true; Key = 'context7' } }
        'blender' { return [ordered]@{ Enabled = $true; Key = 'blender' } }
        'freecad' { return [ordered]@{ Enabled = $true; Key = 'freecad' } }
        default {
            $normalized = $ServerName -replace '^[^A-Za-z0-9]+', '' -replace '[^A-Za-z0-9_-]+', '-'
            if ([string]::IsNullOrWhiteSpace($normalized)) {
                $normalized = 'server'
            }
            return [ordered]@{ Enabled = $true; Key = $normalized.ToLowerInvariant() }
        }
    }
}

function ConvertTo-PowerShellSingleQuotedLiteral([string]$Value) {
    if ($null -eq $Value) {
        return "''"
    }

    return "'" + $Value.Replace("'", "''") + "'"
}

function ConvertTo-ClaudeWrappedStdioCommand([string]$Command, [string[]]$Arguments, [System.Collections.IDictionary]$Environment) {
    if ($null -eq $Environment -or $Environment.Count -eq 0) {
        return [ordered]@{
            Command = $Command
            Args = @($Arguments)
        }
    }

    $scriptLines = New-Object System.Collections.Generic.List[string]
    foreach ($envName in $Environment.Keys) {
        $scriptLines.Add(('`$env:{0} = {1}' -f $envName, (ConvertTo-PowerShellSingleQuotedLiteral ([string]$Environment[$envName]))))
    }

    $commandSegments = New-Object System.Collections.Generic.List[string]
    $commandSegments.Add('&')
    $commandSegments.Add((ConvertTo-PowerShellSingleQuotedLiteral $Command))
    foreach ($argument in @($Arguments)) {
        $commandSegments.Add((ConvertTo-PowerShellSingleQuotedLiteral ([string]$argument)))
    }
    $scriptLines.Add(($commandSegments -join ' '))

    return [ordered]@{
        Command = 'powershell'
        Args = @('-NoProfile', '-Command', ($scriptLines -join '; '))
    }
}

function Get-CopilotCliTransport([System.Collections.IDictionary]$Config) {
    if ($Config.Contains('type')) {
        switch ([string]$Config['type']) {
            'http' { return 'http' }
            'sse' { return 'sse' }
            'stdio' { return 'local' }
            'local' { return 'local' }
            default { return [string]$Config['type'] }
        }
    }

    if ($Config.Contains('url')) { return 'http' }
    if ($Config.Contains('command')) { return 'local' }

    return 'local'
}

function ConvertTo-CopilotCliMcpConfig([System.Collections.IDictionary]$Config) {
    $transport = Get-CopilotCliTransport $Config
    $converted = [ordered]@{
        type = $transport
        tools = @('*')
    }

    if ($transport -eq 'http' -or $transport -eq 'sse') {
        if ($Config.Contains('url')) {
            $converted['url'] = [string]$Config['url']
        }
        if ($Config.Contains('headers') -and $Config['headers'] -is [System.Collections.IDictionary]) {
            $converted['headers'] = $Config['headers']
        }
        return $converted
    }

    if ($Config.Contains('command')) {
        $converted['command'] = [string]$Config['command']
    }

    $normalizedArgs = [System.Collections.Generic.List[object]]::new()
    if ($Config.Contains('args') -and $null -ne $Config['args']) {
        if ($Config['args'] -is [System.Collections.IEnumerable] -and -not ($Config['args'] -is [string])) {
            foreach ($arg in $Config['args']) {
                $normalizedArgs.Add([string]$arg)
            }
        }
        else {
            $normalizedArgs.Add([string]$Config['args'])
        }
    }
    $converted['args'] = [object[]]$normalizedArgs.ToArray()

    if ($Config.Contains('env') -and $Config['env'] -is [System.Collections.IDictionary]) {
        $converted['env'] = $Config['env']
    }

    return $converted
}

function ConvertTo-OpenCodeMcpConfig([System.Collections.IDictionary]$Config) {
    $transport = if ($Config.Contains('type')) { [string]$Config['type'] } elseif ($Config.Contains('url')) { 'http' } else { 'stdio' }
    if ($transport -eq 'http' -or $transport -eq 'sse') {
        $converted = [ordered]@{
            type = 'remote'
            url = [string]$Config['url']
            enabled = $true
        }
        if ($Config.Contains('headers') -and $Config['headers'] -is [System.Collections.IDictionary]) {
            $converted['headers'] = $Config['headers']
        }
        if ($Config.Contains('timeout')) {
            $converted['timeout'] = $Config['timeout']
        }
        return $converted
    }

    $command = [System.Collections.Generic.List[object]]::new()
    if ($Config.Contains('command')) {
        $command.Add([string]$Config['command'])
    }
    if ($Config.Contains('args') -and $Config['args'] -is [System.Collections.IEnumerable] -and -not ($Config['args'] -is [string])) {
        foreach ($arg in $Config['args']) {
            $command.Add([string]$arg)
        }
    }
    elseif ($Config.Contains('args') -and $null -ne $Config['args']) {
        $command.Add([string]$Config['args'])
    }

    $converted = [ordered]@{
        type = 'local'
        command = [object[]]$command.ToArray()
        enabled = $true
    }
    if ($Config.Contains('env') -and $Config['env'] -is [System.Collections.IDictionary]) {
        $converted['environment'] = $Config['env']
    }
    if ($Config.Contains('timeout')) {
        $converted['timeout'] = $Config['timeout']
    }
    return $converted
}

function ConvertTo-TomlString([string]$Value) {
    $escaped = $Value.Replace('\', '\\').Replace('"', '\"')
    return '"' + $escaped + '"'
}

function Format-TomlKeySegment([string]$Key) {
    if ($Key -match '^[A-Za-z0-9_-]+$') {
        return $Key
    }

    return (ConvertTo-TomlString $Key)
}

function ConvertTo-TomlValue([object]$Value) {
    if ($null -eq $Value) {
        return '""'
    }

    if ($Value -is [string]) {
        return ConvertTo-TomlString $Value
    }

    if ($Value -is [bool]) {
        return $Value.ToString().ToLowerInvariant()
    }

    if ($Value -is [ValueType]) {
        return [System.Convert]::ToString($Value, [System.Globalization.CultureInfo]::InvariantCulture)
    }

    if ($Value -is [System.Collections.IEnumerable] -and -not ($Value -is [string])) {
        return '[' + ((@($Value) | ForEach-Object { ConvertTo-TomlValue $_ }) -join ', ') + ']'
    }

    return ConvertTo-TomlString ([string]$Value)
}

function ConvertTo-TomlTableSections([string[]]$PathSegments, [System.Collections.IDictionary]$Data) {
    $header = '[' + (($PathSegments | ForEach-Object { Format-TomlKeySegment $_ }) -join '.') + ']'
    $lines = @($header)
    $nestedSections = @()

    foreach ($key in $Data.Keys) {
        $value = $Data[$key]
        if ($value -is [System.Collections.IDictionary]) {
            $nestedSections += ConvertTo-TomlTableSections -PathSegments ($PathSegments + @($key)) -Data $value
            continue
        }

        $lines += ('{0} = {1}' -f $key, (ConvertTo-TomlValue $value))
    }

    return @(($lines -join "`r`n")) + @($nestedSections)
}

function ConvertTo-CodexMcpSection([string]$ServerName, [System.Collections.IDictionary]$Config) {
    return (ConvertTo-TomlTableSections -PathSegments @('mcp_servers', $ServerName) -Data $Config) -join "`r`n`r`n"
}

function Get-CodexTableServerName([string]$Line) {
    if ($Line -notmatch '^\[(?<path>[^\]]+)\]\s*$') { return $null }

    $path = $matches['path']
    if (-not $path.StartsWith('mcp_servers.')) { return $null }

    $remainder = $path.Substring('mcp_servers.'.Length)
    if ([string]::IsNullOrWhiteSpace($remainder)) { return $null }

    if ($remainder.StartsWith('"')) {
        $builder = New-Object System.Text.StringBuilder
        for ($index = 1; $index -lt $remainder.Length; $index++) {
            $char = $remainder[$index]
            if ($char -eq '"' -and $remainder[$index - 1] -ne '\\') {
                return $builder.ToString().Replace('\\"', '"').Replace('\\\\', '\\')
            }
            [void]$builder.Append($char)
        }

        return $null
    }

    if ($remainder -match '^(?<name>[^\.]+)') {
        return $matches['name']
    }

    return $remainder
}

function Remove-CodexManagedServersFromToml([string]$RawContent, [string[]]$ServerNames) {
    if ([string]::IsNullOrWhiteSpace($RawContent)) {
        return ''
    }

    $managedNames = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($serverName in $ServerNames) {
        [void]$managedNames.Add($serverName)
    }

    $result = New-Object System.Collections.Generic.List[string]
    $skipCurrentSection = $false

    foreach ($line in ($RawContent -split "`r?`n")) {
        $serverName = Get-CodexTableServerName $line
        if ($null -ne $serverName) {
            $skipCurrentSection = $managedNames.Contains($serverName)
        }
        elseif ($line -match '^\[[^\]]+\]\s*$') {
            $skipCurrentSection = $false
        }

        if (-not $skipCurrentSection) {
            [void]$result.Add($line)
        }
    }

    return ($result -join "`r`n").TrimEnd("`r", "`n")
}

function Get-LegacyManagedMcpAliases([string]$RuntimeName, [string]$ServerName) {
    switch ($RuntimeName) {
        'vscode' {
            switch ($ServerName) {
                'github' { return @('github-mcp-server') }
            }
        }
        'copilot-cli' {
            switch ($ServerName) {
                'github' { return @('github-mcp-server') }
                'playwright' { return @('microsoft/playwright-mcp', 'microsoft-playwright-mcp', 'playwright-mcp') }
            }
        }
        'gemini' {
            switch ($ServerName) {
                'github' { return @('github-mcp-server', 'github/github-mcp-server') }
                'upstash/context7' { return @('context7') }
                'microsoftdocs/mcp' { return @('Microsoft Learn MCP Server') }
                'github/github-mcp-server' { return @('github') }
                'chromedevtools/chrome-devtools-mcp' { return @('chrome-devtools') }
                'playwright' { return @('microsoft/playwright-mcp', 'microsoft-playwright-mcp', 'playwright-mcp') }
            }
        }
        'antigravity' {
            switch ($ServerName) {
                'github' { return @('github-mcp-server') }
            }
        }
        'codex' {
            switch ($ServerName) {
                'github' { return @('github-mcp-server', 'github/github-mcp-server') }
                'upstash/context7' { return @('context7') }
                'microsoftdocs/mcp' { return @('microsoftdocs') }
                'imageFetch' { return @('imagefetch') }
                'github/github-mcp-server' { return @('github') }
                'chromedevtools/chrome-devtools-mcp' { return @('chrome-devtools') }
                'playwright' { return @('microsoft/playwright-mcp', 'microsoft-playwright-mcp', 'playwright-mcp') }
            }
        }
        'claude' {
            switch ($ServerName) {
                'github' { return @('github-mcp-server') }
            }
        }
    }

    return @()
}

function Get-DeprecatedManagedMcpDefinitions {
    return [ordered]@{
        memory = [ordered]@{
            RawConfig = [ordered]@{
                command = 'npx'
                args = @('-y', '@modelcontextprotocol/server-memory')
                env = [ordered]@{
                    MEMORY_FILE_PATH = Join-Path $env:USERPROFILE 'mcp-memory.json'
                }
            }
            Match = [ordered]@{
                command = 'npx'
                args = @('-y', '@modelcontextprotocol/server-memory')
            }
        }
    }
}

function Get-DeprecatedManagedMcpKeys {
    return @((Get-DeprecatedManagedMcpDefinitions).Keys)
}

function Get-DeprecatedManagedMcpConfig([string]$RuntimeName, [string]$Key) {
    $definitions = Get-DeprecatedManagedMcpDefinitions
    if (-not $definitions.Contains($Key)) {
        return $null
    }

    $rawConfig = ConvertTo-OrderedMap $definitions[$Key]['RawConfig']
    switch ($RuntimeName) {
        'vscode' { return $rawConfig }
        'copilot-cli' { return ConvertTo-CopilotCliMcpConfig $rawConfig }
        default { return $null }
    }
}

function Test-IsDeprecatedManagedMcpEntry([string]$RuntimeName, [string]$Key, [object]$ExistingConfig) {
    if ($null -eq $ExistingConfig) {
        return $false
    }

    $definitions = Get-DeprecatedManagedMcpDefinitions
    if (-not $definitions.Contains($Key)) {
        return $false
    }

    $expectedMatch = ConvertTo-OrderedMap $definitions[$Key]['Match']
    $config = ConvertTo-OrderedMap $ExistingConfig
    if ($RuntimeName -eq 'copilot-cli') {
        if (-not $config.Contains('command') -or -not $config.Contains('args')) {
            return $false
        }

        if ($config.Contains('type') -and [string]$config['type'] -ne 'local') {
            return $false
        }
    }
    elseif (-not $config.Contains('command') -or -not $config.Contains('args')) {
        return $false
    }

    $command = [string]$config['command']
    $args = @($config['args'] | ForEach-Object { [string]$_ })
    $expectedArgs = @($expectedMatch['args'] | ForEach-Object { [string]$_ })
    return $command -eq [string]$expectedMatch['command'] -and (Test-JsonLikeEqual $args $expectedArgs)
}

function Get-ResolvedManagedMcpManifest {
    $context = $script:SetupContext
    if (-not (Test-Path $context.McpSourceFile)) {
        Write-Host "  [WARN] MCP source file not found: $($context.McpSourceFile)" -ForegroundColor Yellow
        return $null
    }

    if (-not (Test-Path $context.McpLocalFile) -and -not $script:SetupOptions.Uninstall -and -not $script:SetupOptions.DryRun) {
        Write-JsonOrderedMap $context.McpLocalFile ([ordered]@{ servers = [ordered]@{} })
        Write-Host "  [OK] Created local MCP override file: $($context.McpLocalFile)"
    }

    $manifest = Read-JsonOrderedMap $context.McpSourceFile
    if ($null -eq $manifest) { return $null }

    if (Test-Path $context.McpLocalFile) {
        $localManifest = Read-JsonOrderedMap $context.McpLocalFile
        if ($null -eq $localManifest) { return $null }
        $manifest = Merge-OrderedMap $manifest $localManifest
    }

    if (-not $manifest.Contains('servers') -or $manifest['servers'] -isnot [System.Collections.IDictionary]) {
        Write-Host '  [WARN] MCP manifest does not contain a valid servers object.' -ForegroundColor Yellow
        return $null
    }

    $machineConfig = Get-GalMachineConfig
    if ($null -eq $machineConfig) { return $null }

    $envFile = Join-Path $context.GalConfigRoot 'config.local.env'
    $legacyEnvFile = Join-Path $context.RepoRoot 'config.local.env'
    if (-not (Test-Path $envFile) -and (Test-Path $legacyEnvFile)) {
        $envFile = $legacyEnvFile
    }

    $mcpVariables = Get-McpVariableMap (Read-KeyValueEnvFile $envFile) $machineConfig
    $resolvedServers = [ordered]@{}
    foreach ($serverName in $manifest['servers'].Keys) {
        $resolvedServers[$serverName] = Resolve-McpConfig $manifest['servers'][$serverName] $mcpVariables
    }

    $runtimeManifest = [ordered]@{
        servers = (Normalize-McpServers $resolvedServers)
    }

    if ($manifest.Contains('inputs')) {
        $runtimeManifest['inputs'] = Resolve-McpInputs $manifest['inputs'] $mcpVariables
    }

    return [ordered]@{
        RuntimeManifest = $runtimeManifest
        McpValues = $mcpVariables
    }
}

function Update-VscodeMcpConfig([System.Collections.IDictionary]$ManagedManifest, [System.Collections.IDictionary]$PreviousProjection) {
    $context = $script:SetupContext
    $vscodeMcp = Read-JsonOrderedMap $context.VscodeMcpFile
    if ($null -eq $vscodeMcp) { return }
    $previousRuntimeEntries = Get-ProjectionRuntimeEntries -Projection $PreviousProjection -RuntimeName 'vscode'
    $managedRuntimeEntries = [ordered]@{}

    if (-not $vscodeMcp.Contains('servers') -or $vscodeMcp['servers'] -isnot [System.Collections.IDictionary]) {
        $vscodeMcp['servers'] = [ordered]@{}
    }

    $changed = $false
    foreach ($deprecatedKey in (Get-DeprecatedManagedMcpKeys)) {
        if (-not $vscodeMcp['servers'].Contains($deprecatedKey)) {
            continue
        }

        $deprecatedConfig = Get-DeprecatedManagedMcpConfig -RuntimeName 'vscode' -Key $deprecatedKey
        if (-not (Test-IsDeprecatedManagedMcpEntry -RuntimeName 'vscode' -Key $deprecatedKey -ExistingConfig $vscodeMcp['servers'][$deprecatedKey]) -and -not (Test-CanReplaceManagedRuntimeEntry -PreviousRuntimeEntries $previousRuntimeEntries -Key $deprecatedKey -ExistingConfig $vscodeMcp['servers'][$deprecatedKey] -DesiredConfig $deprecatedConfig -RuntimeLabel 'VS Code')) {
            continue
        }

        $vscodeMcp['servers'].Remove($deprecatedKey)
        $changed = $true
        if ($script:SetupOptions.DryRun) {
            Write-Host "  [DRY RUN] Would remove deprecated VS Code MCP server: $deprecatedKey"
        }
        else {
            Write-Host "  [CLEANUP] Deprecated VS Code MCP server removed: $deprecatedKey"
        }
    }

    foreach ($serverName in $ManagedManifest['servers'].Keys) {
        $resolvedConfig = $ManagedManifest['servers'][$serverName]
        foreach ($legacyAlias in (Get-LegacyManagedMcpAliases -RuntimeName 'vscode' -ServerName $serverName)) {
            if (
                $vscodeMcp['servers'].Contains($legacyAlias) -and
                (Test-CanReplaceManagedRuntimeEntry -PreviousRuntimeEntries $previousRuntimeEntries -Key $legacyAlias -ExistingConfig $vscodeMcp['servers'][$legacyAlias] -DesiredConfig $resolvedConfig -RuntimeLabel 'VS Code')
            ) {
                $vscodeMcp['servers'].Remove($legacyAlias)
                $changed = $true
                if ($script:SetupOptions.DryRun) {
                    Write-Host "  [DRY RUN] Would remove VS Code MCP alias: $legacyAlias"
                }
                else {
                    Write-Host "  [CLEANUP] VS Code MCP alias removed: $legacyAlias"
                }
            }
        }
    }
    foreach ($serverName in $ManagedManifest['servers'].Keys) {
        $resolvedConfig = $ManagedManifest['servers'][$serverName]
        $existingConfig = if ($vscodeMcp['servers'].Contains($serverName)) { $vscodeMcp['servers'][$serverName] } else { $null }
        if (-not (Test-CanReplaceManagedRuntimeEntry -PreviousRuntimeEntries $previousRuntimeEntries -Key $serverName -ExistingConfig $existingConfig -DesiredConfig $resolvedConfig -RuntimeLabel 'VS Code')) {
            continue
        }

        if (-not $vscodeMcp['servers'].Contains($serverName) -or -not (Test-JsonLikeEqual $existingConfig $resolvedConfig)) {
            $vscodeMcp['servers'][$serverName] = $resolvedConfig
            $changed = $true
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would set VS Code MCP server: $serverName"
            }
            else {
                Write-Host "  [SET] VS Code MCP server: $serverName"
            }
        }

        if ($null -eq $existingConfig -or $previousRuntimeEntries.Contains($serverName)) {
            $managedRuntimeEntries[$serverName] = ConvertTo-OrderedMap $resolvedConfig
        }
    }

    if (Sync-ManagedMcpInputs -Data $vscodeMcp -ManagedManifest $ManagedManifest) {
        $changed = $true
        if ($script:SetupOptions.DryRun) {
            Write-Host '  [DRY RUN] Would sync VS Code MCP inputs'
        }
        else {
            Write-Host '  [SET] VS Code MCP inputs'
        }
    }

    if ($changed -and -not $script:SetupOptions.DryRun) {
        Write-JsonOrderedMap $context.VscodeMcpFile $vscodeMcp
        Write-Host "  [OK] $($context.VscodeMcpFile)"
    }

    return $managedRuntimeEntries
}

function Update-CopilotCliMcpConfig([System.Collections.IDictionary]$ManagedManifest, [System.Collections.IDictionary]$PreviousProjection) {
    $context = $script:SetupContext
    $copilotCliMcp = Read-JsonOrderedMap $context.CopilotCliMcpFile
    if ($null -eq $copilotCliMcp) { return }
    $previousRuntimeEntries = Get-ProjectionRuntimeEntries -Projection $PreviousProjection -RuntimeName 'copilot-cli'
    $managedRuntimeEntries = [ordered]@{}

    if (-not $copilotCliMcp.Contains('mcpServers') -or $copilotCliMcp['mcpServers'] -isnot [System.Collections.IDictionary]) {
        $copilotCliMcp['mcpServers'] = [ordered]@{}
    }

    $managedBridgeConfigs = foreach ($serverName in $ManagedManifest['servers'].Keys) {
        Get-CopilotCliBridgeProfile -ServerName $serverName
    }

    $managedKeys = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($bridgeConfig in @($managedBridgeConfigs)) {
        if ($null -ne $bridgeConfig['Key'] -and -not [string]::IsNullOrWhiteSpace([string]$bridgeConfig['Key'])) {
            [void]$managedKeys.Add([string]$bridgeConfig['Key'])
        }
    }
    foreach ($legacyName in @('chromedevtools/chrome-devtools-mcp', 'github-mcp-server', 'microsoftdocs/mcp', 'microsoft/markitdown', 'upstash/context7')) {
        [void]$managedKeys.Add($legacyName)
    }
    foreach ($deprecatedKey in (Get-DeprecatedManagedMcpKeys)) {
        [void]$managedKeys.Add($deprecatedKey)
    }
    foreach ($serverName in $ManagedManifest['servers'].Keys) {
        foreach ($legacyAlias in (Get-LegacyManagedMcpAliases -RuntimeName 'copilot-cli' -ServerName $serverName)) {
            [void]$managedKeys.Add($legacyAlias)
        }
    }

    $changed = $false
    foreach ($existingServerName in @($copilotCliMcp['mcpServers'].Keys)) {
        if ($managedKeys.Contains([string]$existingServerName)) {
            $desiredConfig = Find-CopilotCliManagedConfigForKey -ManagedManifest $ManagedManifest -Key ([string]$existingServerName)
            if ($null -eq $desiredConfig) {
                $desiredConfig = Get-DeprecatedManagedMcpConfig -RuntimeName 'copilot-cli' -Key ([string]$existingServerName)
            }
            if (-not (Test-IsDeprecatedManagedMcpEntry -RuntimeName 'copilot-cli' -Key ([string]$existingServerName) -ExistingConfig $copilotCliMcp['mcpServers'][$existingServerName]) -and -not (Test-CanReplaceManagedRuntimeEntry -PreviousRuntimeEntries $previousRuntimeEntries -Key ([string]$existingServerName) -ExistingConfig $copilotCliMcp['mcpServers'][$existingServerName] -DesiredConfig $desiredConfig -RuntimeLabel 'Copilot CLI')) {
                continue
            }

            $copilotCliMcp['mcpServers'].Remove($existingServerName)
            $changed = $true
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would remove Copilot CLI MCP server: $existingServerName"
            }
            else {
                Write-Host "  [CLEANUP] Copilot CLI MCP server removed: $existingServerName"
            }
        }
    }

    foreach ($serverName in $ManagedManifest['servers'].Keys) {
        $bridgeConfig = Get-CopilotCliBridgeProfile -ServerName $serverName
        if (-not $bridgeConfig['Enabled']) {
            continue
        }

        $targetServerName = [string]$bridgeConfig['Key']
        $converted = ConvertTo-CopilotCliMcpConfig $ManagedManifest['servers'][$serverName]
        $existingConfig = if ($copilotCliMcp['mcpServers'].Contains($targetServerName)) { $copilotCliMcp['mcpServers'][$targetServerName] } else { $null }
        if (-not (Test-CanReplaceManagedRuntimeEntry -PreviousRuntimeEntries $previousRuntimeEntries -Key $targetServerName -ExistingConfig $existingConfig -DesiredConfig $converted -RuntimeLabel 'Copilot CLI')) {
            continue
        }

        if (-not $copilotCliMcp['mcpServers'].Contains($targetServerName) -or -not (Test-JsonLikeEqual $existingConfig $converted)) {
            $copilotCliMcp['mcpServers'][$targetServerName] = $converted
            $changed = $true
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would set Copilot CLI MCP server: $targetServerName"
            }
            else {
                Write-Host "  [SET] Copilot CLI MCP server: $targetServerName"
            }
        }

        if ($null -eq $existingConfig -or $previousRuntimeEntries.Contains($targetServerName)) {
            $managedRuntimeEntries[$targetServerName] = ConvertTo-OrderedMap $converted
        }
    }

    if ($changed -and -not $script:SetupOptions.DryRun) {
        Write-JsonOrderedMap $context.CopilotCliMcpFile $copilotCliMcp
        Write-Host "  [OK] $($context.CopilotCliMcpFile)"
    }

    return $managedRuntimeEntries
}

function Update-AgyMcpConfig([System.Collections.IDictionary]$ManagedManifest) {
    $context = $script:SetupContext

    # --- Write MCP config to the AGY plugin root ---
    $pluginMcpFile = Join-Path $context.AgyPluginInstallTarget 'mcp_config.json'
    $pluginDir = Split-Path $pluginMcpFile -Parent

    # Build AGY-format MCP config from the managed manifest
    $agyServers = [ordered]@{}
    foreach ($serverName in $ManagedManifest['servers'].Keys) {
        $agyServers[$serverName] = ConvertTo-AgyMcpConfig $ManagedManifest['servers'][$serverName]
    }
    $pluginMcpConfig = [ordered]@{
        mcpServers = $agyServers
    }
    if ($ManagedManifest.Contains('inputs')) {
        $pluginMcpConfig['inputs'] = $ManagedManifest['inputs']
    }

    if (-not $script:SetupOptions.DryRun) {
        if (-not (Test-Path $pluginDir)) {
            New-Item -ItemType Directory -Path $pluginDir -Force | Out-Null
        }
        Write-JsonOrderedMap $pluginMcpFile $pluginMcpConfig
        Write-Host "  [SET] AGY plugin MCP config: $pluginMcpFile"
    }
    else {
        Write-Host "  [DRY RUN] Would write AGY plugin MCP config: $pluginMcpFile"
    }

    # --- Clean up GAL-managed entries from the global AGY MCP config ---
    $agyConfig = Read-JsonOrderedMap $context.AntigravityMcpFile
    if ($null -eq $agyConfig) { return }

    if (-not $agyConfig.Contains('mcpServers') -or $agyConfig['mcpServers'] -isnot [System.Collections.IDictionary]) {
        return
    }

    $changed = $false
    foreach ($serverName in $ManagedManifest['servers'].Keys) {
        # Remove the canonical name
        if ($agyConfig['mcpServers'].Contains($serverName)) {
            $agyConfig['mcpServers'].Remove($serverName)
            $changed = $true
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would remove global AGY MCP entry: $serverName"
            }
            else {
                Write-Host "  [CLEANUP] Global AGY MCP entry removed: $serverName"
            }
        }
        # Remove legacy aliases
        foreach ($legacyAlias in (Get-LegacyManagedMcpAliases -RuntimeName 'antigravity' -ServerName $serverName)) {
            if ($agyConfig['mcpServers'].Contains($legacyAlias)) {
                $agyConfig['mcpServers'].Remove($legacyAlias)
                $changed = $true
                if ($script:SetupOptions.DryRun) {
                    Write-Host "  [DRY RUN] Would remove global AGY MCP alias: $legacyAlias"
                }
                else {
                    Write-Host "  [CLEANUP] Global AGY MCP alias removed: $legacyAlias"
                }
            }
        }
    }

    # Remove managed inputs from global config
    if (Remove-ManagedMcpInputs -Data $agyConfig -ManagedManifest $ManagedManifest) {
        $changed = $true
        if ($script:SetupOptions.DryRun) {
            Write-Host '  [DRY RUN] Would remove global AGY MCP inputs'
        }
        else {
            Write-Host '  [CLEANUP] Global AGY MCP inputs removed'
        }
    }

    # Clean up empty mcpServers block
    if ($agyConfig['mcpServers'].Count -eq 0) {
        $agyConfig.Remove('mcpServers')
        $changed = $true
    }

    if ($changed -and -not $script:SetupOptions.DryRun) {
        Write-JsonOrderedMap $context.AntigravityMcpFile $agyConfig
        Write-Host "  [OK] $($context.AntigravityMcpFile)"
    }
}

function Remove-LegacyGeminiMcpConfig([System.Collections.IDictionary]$ManagedManifest) {
    $context = $script:SetupContext
    if (-not (Test-Path $context.GeminiSettingsFile)) {
        return
    }

    $geminiSettings = Read-JsonOrderedMap $context.GeminiSettingsFile
    if ($null -eq $geminiSettings) {
        return
    }

    $changed = $false
    if ($geminiSettings.Contains('mcpServers') -and $geminiSettings['mcpServers'] -is [System.Collections.IDictionary]) {
        foreach ($serverName in $ManagedManifest['servers'].Keys) {
            foreach ($managedName in @($serverName) + @(Get-LegacyManagedMcpAliases -RuntimeName 'gemini' -ServerName $serverName)) {
                if ($geminiSettings['mcpServers'].Contains($managedName)) {
                    $geminiSettings['mcpServers'].Remove($managedName)
                    $changed = $true
                    if ($script:SetupOptions.DryRun) {
                        Write-Host "  [DRY RUN] Would remove legacy Gemini MCP entry: $managedName"
                    }
                    else {
                        Write-Host "  [CLEANUP] Legacy Gemini MCP entry removed: $managedName"
                    }
                }
            }
        }

        if ($geminiSettings['mcpServers'].Count -eq 0) {
            $geminiSettings.Remove('mcpServers')
            $changed = $true
            if ($script:SetupOptions.DryRun) {
                Write-Host '  [DRY RUN] Would remove empty legacy Gemini mcpServers block'
            }
            else {
                Write-Host '  [CLEANUP] Empty legacy Gemini mcpServers block removed'
            }
        }
    }

    if (Remove-ManagedMcpInputs -Data $geminiSettings -ManagedManifest $ManagedManifest) {
        $changed = $true
        if ($script:SetupOptions.DryRun) {
            Write-Host '  [DRY RUN] Would remove legacy Gemini MCP inputs'
        }
        else {
            Write-Host '  [CLEANUP] Legacy Gemini MCP inputs removed'
        }
    }

    if ($changed -and -not $script:SetupOptions.DryRun) {
        Write-JsonOrderedMap $context.GeminiSettingsFile $geminiSettings
        Write-Host "  [OK] $($context.GeminiSettingsFile)"
    }
}

function Update-CodexMcpConfig([System.Collections.IDictionary]$ManagedManifest) {
    $context = $script:SetupContext
    $codexConfigDir = Split-Path $context.CodexConfigFile -Parent
    if (-not (Test-Path $codexConfigDir) -and -not $script:SetupOptions.DryRun) {
        New-Item -ItemType Directory -Path $codexConfigDir -Force | Out-Null
    }

    $codexRaw = if (Test-Path $context.CodexConfigFile) { Get-Content $context.CodexConfigFile -Raw -Encoding UTF8 } else { '' }
    $managedNames = New-Object System.Collections.Generic.List[string]
    foreach ($serverName in $ManagedManifest['servers'].Keys) {
        [void]$managedNames.Add($serverName)
        $bridgeConfig = Get-CodexBridgeProfile -ServerName $serverName
        if (-not [string]::IsNullOrWhiteSpace($bridgeConfig['Key'])) {
            [void]$managedNames.Add([string]$bridgeConfig['Key'])
        }
        foreach ($legacyAlias in (Get-LegacyManagedMcpAliases -RuntimeName 'codex' -ServerName $serverName)) {
            [void]$managedNames.Add($legacyAlias)
        }
    }

    $remainingRaw = Remove-CodexManagedServersFromToml -RawContent $codexRaw -ServerNames @($managedNames)
    $sections = foreach ($serverName in $ManagedManifest['servers'].Keys) {
        $bridgeConfig = Get-CodexBridgeProfile -ServerName $serverName
        if (-not $bridgeConfig['Enabled']) { continue }
        ConvertTo-CodexMcpSection -ServerName ([string]$bridgeConfig['Key']) -Config (ConvertTo-CodexMcpConfig $ManagedManifest['servers'][$serverName])
    }

    $newCodexRaw = $remainingRaw
    if (-not [string]::IsNullOrWhiteSpace($newCodexRaw) -and -not $newCodexRaw.EndsWith("`r`n`r`n")) {
        $newCodexRaw = $newCodexRaw.TrimEnd("`r", "`n") + "`r`n`r`n"
    }
    $newCodexRaw += ($sections -join "`r`n`r`n") + "`r`n"

    if ($script:SetupOptions.DryRun) {
        foreach ($serverName in $ManagedManifest['servers'].Keys) {
            $bridgeConfig = Get-CodexBridgeProfile -ServerName $serverName
            if (-not $bridgeConfig['Enabled']) { continue }
            Write-Host "  [DRY RUN] Would set Codex MCP server: $($bridgeConfig['Key'])"
        }
        return
    }

    [System.IO.File]::WriteAllText($context.CodexConfigFile, $newCodexRaw, $context.Utf8NoBom)
    foreach ($serverName in $ManagedManifest['servers'].Keys) {
        $bridgeConfig = Get-CodexBridgeProfile -ServerName $serverName
        if (-not $bridgeConfig['Enabled']) { continue }
        Write-Host "  [SET] Codex MCP server: $($bridgeConfig['Key'])"
    }
    Write-Host "  [OK] $($context.CodexConfigFile)"
}

function Update-OpenCodeMcpConfig([System.Collections.IDictionary]$ManagedManifest) {
    $context = $script:SetupContext
    $openCodeConfig = Read-JsonOrderedMap $context.OpenCodeConfigFile
    if ($null -eq $openCodeConfig) {
        $openCodeConfig = [ordered]@{}
    }

    if (-not $openCodeConfig.Contains('mcp') -or $openCodeConfig['mcp'] -isnot [System.Collections.IDictionary]) {
        $openCodeConfig['mcp'] = [ordered]@{}
    }

    $changed = $false
    foreach ($serverName in $ManagedManifest['servers'].Keys) {
        $converted = ConvertTo-OpenCodeMcpConfig $ManagedManifest['servers'][$serverName]
        if (-not $openCodeConfig['mcp'].Contains($serverName) -or -not (Test-JsonLikeEqual $openCodeConfig['mcp'][$serverName] $converted)) {
            $openCodeConfig['mcp'][$serverName] = $converted
            $changed = $true
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would set OpenCode MCP server: $serverName"
            }
            else {
                Write-Host "  [SET] OpenCode MCP server: $serverName"
            }
        }
    }

    if ($changed -and -not $script:SetupOptions.DryRun) {
        Write-JsonOrderedMap $context.OpenCodeConfigFile $openCodeConfig
        Write-Host "  [OK] $($context.OpenCodeConfigFile)"
    }
}

function Invoke-ClaudeMcpCommand([string[]]$Arguments) {
    & claude @Arguments
    return $LASTEXITCODE
}

function Test-ClaudeMcpServerRegistered([string]$ServerName) {
    $output = & claude mcp list 2>&1
    if ($LASTEXITCODE -ne 0) {
        return $false
    }

    $escapedName = [regex]::Escape($ServerName)
    foreach ($line in @($output)) {
        if ([string]$line -match ('^{0}:' -f $escapedName)) {
            return $true
        }
    }

    return $false
}

function Update-ClaudeMcpConfig([System.Collections.IDictionary]$ManagedManifest) {
    if (-not (Test-CommandAvailable 'claude')) {
        Write-Host '  [WARN] Claude CLI not found; skipping Claude MCP installation.' -ForegroundColor Yellow
        return
    }

    foreach ($serverName in $ManagedManifest['servers'].Keys) {
        $bridgeConfig = Get-ClaudeBridgeProfile -ServerName $serverName
        if (-not $bridgeConfig['Enabled']) {
            continue
        }

        $bridgeKey = [string]$bridgeConfig['Key']
        $resolvedConfig = $ManagedManifest['servers'][$serverName]
        if ($script:SetupOptions.DryRun) {
            Write-Host "  [DRY RUN] Would upsert Claude MCP server: $bridgeKey"
            continue
        }

        foreach ($legacyAlias in (Get-LegacyManagedMcpAliases -RuntimeName 'claude' -ServerName $serverName)) {
            Invoke-ClaudeMcpCommand -Arguments @('mcp', 'remove', $legacyAlias) | Out-Null
        }
        if (-not [string]::Equals($bridgeKey, $serverName, [System.StringComparison]::OrdinalIgnoreCase)) {
            Invoke-ClaudeMcpCommand -Arguments @('mcp', 'remove', $serverName) | Out-Null
        }
        Invoke-ClaudeMcpCommand -Arguments @('mcp', 'remove', $bridgeKey) | Out-Null
        Invoke-ClaudeMcpCommand -Arguments @('mcp', 'remove', $serverName) | Out-Null

        $exitCode = 0
        if ($resolvedConfig.Contains('type') -and [string]$resolvedConfig['type'] -eq 'http') {
            $arguments = @('mcp', 'add', '--scope', 'user', '--transport', 'http', $bridgeKey, [string]$resolvedConfig['url'])
            if ($resolvedConfig.Contains('headers') -and $resolvedConfig['headers'] -is [System.Collections.IDictionary]) {
                foreach ($headerName in $resolvedConfig['headers'].Keys) {
                    $arguments += @('--header', ('{0}: {1}' -f $headerName, $resolvedConfig['headers'][$headerName]))
                }
            }
            if ($resolvedConfig.Contains('tools')) {
                Write-Host "  [WARN] Claude CLI does not expose a tools filter flag; installing $bridgeKey without tools scoping." -ForegroundColor Yellow
            }

            $exitCode = Invoke-ClaudeMcpCommand -Arguments $arguments
        }
        else {
            $commandAndArgs = @()
            if ($resolvedConfig.Contains('args')) {
                $commandAndArgs += @($resolvedConfig['args'] | ForEach-Object { [string]$_ })
            }

            $wrappedCommand = ConvertTo-ClaudeWrappedStdioCommand -Command ([string]$resolvedConfig['command']) -Arguments $commandAndArgs -Environment $(if ($resolvedConfig.Contains('env') -and $resolvedConfig['env'] -is [System.Collections.IDictionary]) { $resolvedConfig['env'] } else { $null })
            $arguments = @('mcp', 'add', '--scope', 'user', $bridgeKey, '--', [string]$wrappedCommand['Command']) + @($wrappedCommand['Args'] | ForEach-Object { [string]$_ })
            $exitCode = Invoke-ClaudeMcpCommand -Arguments $arguments
        }

        if ($exitCode -eq 0 -or (Test-ClaudeMcpServerRegistered -ServerName $bridgeKey)) {
            Write-Host "  [SET] Claude MCP server: $bridgeKey"
        }
        else {
            Write-Host "  [WARN] Failed to configure Claude MCP server: $bridgeKey" -ForegroundColor Yellow
        }
    }
}

function Invoke-UpdateMcp {
    $context = $script:SetupContext

    Write-Host ''
    Write-Host '=== MCP config bridge ==='

    if ($script:SetupOptions.Uninstall) {
        # Remove AGY plugin root MCP config
        $pluginMcpFile = Join-Path $context.AgyPluginInstallTarget 'mcp_config.json'
        if (Test-Path $pluginMcpFile) {
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would remove AGY plugin MCP config: $pluginMcpFile"
            }
            else {
                Remove-Item -LiteralPath $pluginMcpFile -Force
                Write-Host "  [CLEANUP] AGY plugin MCP config removed: $pluginMcpFile"
            }
        }
        Write-Host '  [SKIP] Global MCP config files are preserved during uninstall.'
        return
    }

    if ($script:SetupOptions.DryRun) {
        if ($context.InstallCopilot) {
            Write-Host "  [DRY RUN] Would merge MCP servers into: $($context.VscodeMcpFile)"
            Write-Host "  [DRY RUN] Would merge MCP servers into: $($context.CopilotCliMcpFile)"
        }
        if ($context.InstallGemini -or $context.InstallAntigravity) {
            Write-Host "  [DRY RUN] Would merge MCP servers into: $($context.AntigravityMcpFile)"
            Write-Host "  [DRY RUN] Would remove GAL-managed legacy Gemini MCP entries from: $($context.GeminiSettingsFile)"
        }
        if ($context.InstallCodex) { Write-Host "  [DRY RUN] Would merge MCP servers into: $($context.CodexConfigFile)" }
        if ($context.InstallOpenCode) { Write-Host "  [DRY RUN] Would merge MCP servers into: $($context.OpenCodeConfigFile)" }
        if ($context.InstallClaude) { Write-Host '  [DRY RUN] Would merge MCP servers through Claude CLI user scope' }
    }

    $previousProjection = Get-PreviousManagedProjection
    $resolvedState = Get-ResolvedManagedMcpManifest
    if ($null -eq $resolvedState) { return }

    $manifest = $resolvedState['RuntimeManifest']
    $mcpValues = $resolvedState['McpValues']
    $runtimeEntries = [ordered]@{}

    if ($context.InstallCopilot) {
        $runtimeEntries['vscode'] = Update-VscodeMcpConfig -ManagedManifest $manifest -PreviousProjection $previousProjection
        $runtimeEntries['copilot-cli'] = Update-CopilotCliMcpConfig -ManagedManifest $manifest -PreviousProjection $previousProjection
    }
    if ($context.InstallGemini -or $context.InstallAntigravity) {
        Update-AgyMcpConfig -ManagedManifest $manifest
        Remove-LegacyGeminiMcpConfig -ManagedManifest $manifest
    }
    if ($context.InstallCodex) { Update-CodexMcpConfig -ManagedManifest $manifest }
    if ($context.InstallOpenCode) { Update-OpenCodeMcpConfig -ManagedManifest $manifest }
    if ($context.InstallClaude) { Update-ClaudeMcpConfig -ManagedManifest $manifest }

    Write-GeneratedProjectionFiles -ResolvedManifest $manifest -McpValues $mcpValues -RuntimeEntries $runtimeEntries
}

Initialize-SetupSession -EntryScriptPath $MyInvocation.MyCommand.Path -Uninstall:$Uninstall -Replace:$Replace -DryRun:$DryRun -Reconfigure:$Reconfigure -SelectedRuntimes $SelectedRuntimes -PrimaryRuntime $PrimaryRuntime | Out-Null
Invoke-UpdateMcp