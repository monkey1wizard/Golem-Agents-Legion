#Requires -Version 5.1

[CmdletBinding()]
param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')),
    [string]$ConfigPath = (Join-Path $env:USERPROFILE '.gal\config\config.json'),
    [string]$LockfilePath = (Join-Path $env:USERPROFILE '.gal\state\plugins.lock.json'),
    [string[]]$SelectedRuntimes,
    [string]$PrimaryRuntime,
    [switch]$BootstrapInstall,
    [switch]$DryRun,
    [switch]$Uninstall,
    [switch]$Purge,
    [switch]$ConfirmPurge,
    [switch]$Replace,
    [switch]$Reconfigure,
    [switch]$Force
)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'common' 'Common.ps1')

$script:SetupOptions = [pscustomobject]@{
    Uninstall = $Uninstall.IsPresent
    Replace = $false
    DryRun = $DryRun.IsPresent
    Reconfigure = $false
}

if (-not (Get-Variable -Scope Script -Name SetupContext -ErrorAction SilentlyContinue)) {
    $script:SetupContext = Initialize-SetupSession -EntryScriptPath $MyInvocation.MyCommand.Path -Uninstall:$Uninstall -Replace:$Replace -DryRun:$DryRun -Reconfigure:$Reconfigure -SelectedRuntimes $SelectedRuntimes -PrimaryRuntime $PrimaryRuntime
}

function Get-ProviderFromRuntime([string]$Runtime) {
    switch ($Runtime) {
        'antigravity' { return 'agy' }
        default { return $Runtime }
    }
}

function Get-ProviderLaneForRuntime([string]$Runtime) {
    switch ($Runtime) {
        'opencode' { return 'bridge' }
        'gemini' { return 'migration' }
        default { return 'primary' }
    }
}

function ConvertTo-Bool([object]$Value, [bool]$Default = $false) {
    if ($null -eq $Value) {
        return $Default
    }

    if ($Value -is [bool]) {
        return $Value
    }

    $text = [string]$Value
    if ([string]::IsNullOrWhiteSpace($text)) {
        return $Default
    }

    switch ($text.Trim().ToLowerInvariant()) {
        'true' { return $true }
        '1' { return $true }
        'yes' { return $true }
        'false' { return $false }
        '0' { return $false }
        'no' { return $false }
        default { return $Default }
    }
}

function Get-ResolvedRuntimeSelection {
    param(
        [string[]]$SelectedRuntimes,
        [string]$PrimaryRuntime,
        [pscustomobject]$Context
    )

    $state = Read-JsonOrderedMap $Context.InstallStateFile
    $selected = if (@($SelectedRuntimes).Count -gt 0) {
        @($SelectedRuntimes)
    }
    elseif ($state -and $state.Contains('selectedRuntimes')) {
        @($state['selectedRuntimes'])
    }
    else {
        @('copilot', 'antigravity', 'codex', 'claude')
    }

    $primary = if (-not [string]::IsNullOrWhiteSpace($PrimaryRuntime)) {
        $PrimaryRuntime
    }
    elseif ($state -and $state.Contains('primaryRuntime')) {
        [string]$state['primaryRuntime']
    }
    else {
        $selected | Select-Object -First 1
    }

    return [pscustomobject]@{
        SelectedRuntimes = @($selected | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
        PrimaryRuntime = $primary
    }
}

function New-DefaultGalConfig {
    param(
        [string]$RepoRoot,
        [string[]]$SelectedRuntimes,
        [string]$PrimaryRuntime,
        [switch]$BootstrapInstall
    )

    $providerSelections = [ordered]@{}
    foreach ($runtime in $SelectedRuntimes) {
        $provider = Get-ProviderFromRuntime $runtime
        $providerSelections[$provider] = [ordered]@{
            enabled = $true
            lane = Get-ProviderLaneForRuntime $runtime
        }
    }

    $primaryProvider = Get-ProviderFromRuntime $PrimaryRuntime
    $preferredProviders = [System.Collections.Generic.List[string]]::new()
    if (-not [string]::IsNullOrWhiteSpace($primaryProvider) -and $providerSelections.Contains($primaryProvider)) {
        $preferredProviders.Add($primaryProvider)
    }
    foreach ($providerName in $providerSelections.Keys) {
        if ($preferredProviders -notcontains $providerName) {
            $preferredProviders.Add($providerName)
        }
    }

    $installMode = if ($BootstrapInstall) { 'install' } else { 'source' }
    $galRoot = if ($BootstrapInstall) { '' } else { $RepoRoot }
    $devMode = if ($BootstrapInstall) { $false } else { $true }

    return [ordered]@{
        schemaVersion = 1
        galRoot = $galRoot
        devMode = $devMode
        defaultProfile = 'default'
        profiles = [ordered]@{}
        enabledPlugins = @()
        disabledPlugins = @()
        providerSelections = $providerSelections
        installMode = $installMode
        preferredProviders = @($preferredProviders)
        userSettings = [ordered]@{}
    }
}

function Merge-ProviderSelections {
    param(
        [System.Collections.IDictionary]$Defaults,
        [System.Collections.IDictionary]$Configured
    )

    $merged = [ordered]@{}
    foreach ($providerName in $Defaults.Keys) {
        $merged[$providerName] = ConvertTo-OrderedMap $Defaults[$providerName]
    }

    if ($Configured) {
        foreach ($providerName in $Configured.Keys) {
            if (-not $merged.Contains($providerName)) {
                continue
            }

            $configuredValue = $Configured[$providerName]
            if ($configuredValue -is [System.Collections.IDictionary]) {
                if ($configuredValue.Contains('enabled')) {
                    $merged[$providerName]['enabled'] = ConvertTo-Bool $configuredValue['enabled'] $true
                }
                if ($configuredValue.Contains('lane') -and -not [string]::IsNullOrWhiteSpace([string]$configuredValue['lane'])) {
                    $merged[$providerName]['lane'] = [string]$configuredValue['lane']
                }
            }
            else {
                $merged[$providerName]['enabled'] = ConvertTo-Bool $configuredValue $true
            }
        }
    }

    return $merged
}

function Get-ProviderNamesByLane {
    param(
        [object]$ProviderSelections,
        [string]$Lane,
        [string[]]$FallbackSelectedRuntimes = @()
    )

    $providerNames = [System.Collections.Generic.List[string]]::new()
    $entries = @()

    if ($ProviderSelections -is [System.Collections.IDictionary]) {
        $entries = @($ProviderSelections.GetEnumerator())
    }
    elseif ($null -ne $ProviderSelections) {
        $entries = @($ProviderSelections.PSObject.Properties | ForEach-Object {
            [pscustomobject]@{
                Key = $_.Name
                Value = $_.Value
            }
        })
    }

    foreach ($entry in $entries) {
        $providerName = [string]$entry.Key
        $selection = if ($entry.Value -is [System.Collections.IDictionary]) {
            $entry.Value
        }
        else {
            ConvertTo-OrderedMap $entry.Value
        }

        if ((ConvertTo-Bool $selection['enabled'] $true) -and [string]$selection['lane'] -eq $Lane) {
            $providerNames.Add($providerName)
        }
    }

    if ($providerNames.Count -eq 0 -and $FallbackSelectedRuntimes.Count -gt 0) {
        foreach ($runtime in $FallbackSelectedRuntimes) {
            if ((Get-ProviderLaneForRuntime $runtime) -ne $Lane) {
                continue
            }

            $providerName = Get-ProviderFromRuntime $runtime
            if ($providerNames -notcontains $providerName) {
                $providerNames.Add($providerName)
            }
        }
    }

    return @($providerNames)
}

function Get-DisplayValue([string[]]$Items, [string]$Fallback = 'none') {
    if ($null -eq $Items -or $Items.Count -eq 0) {
        return $Fallback
    }

    return ($Items -join ', ')
}

function Remove-GalManagedDirectory {
    param(
        [Parameter(Mandatory)]
        [string]$Path,
        [Parameter(Mandatory)]
        [string]$Label
    )

    if (-not (Test-Path $Path)) {
        Write-Host ("  [SKIP] No {0} to remove" -f $Label)
        return
    }

    if ($DryRun) {
        Write-Host ("  [DRY RUN] Would remove {0}: {1}" -f $Label, $Path)
        return
    }

    Remove-Item -LiteralPath $Path -Recurse -Force
    Write-Host ("  [REMOVED] {0}: {1}" -f $Label, $Path)
}

function Remove-GalManagedProviderShortcut {
    param(
        [Parameter(Mandatory)]
        [string]$Provider
    )

    $shortcutPath = Get-GalActiveProviderTarget -Provider $Provider
    $shortcutItem = Get-Item -LiteralPath $shortcutPath -Force -ErrorAction SilentlyContinue
    if ($null -eq $shortcutItem) {
        Write-Host ("  [SKIP] No GAL-managed {0} shortcut to remove" -f $Provider)
        return
    }

    Remove-SafeLink $shortcutPath
}

function Test-InstallUninstallOwnership {
    param(
        [pscustomobject]$Context
    )

    $managedTargets = @(
        $Context.InstallStateFile,
        $Context.AgyPluginInstallTarget,
        $Context.GalStorePluginsRoot,
        $Context.GalGeneratedMcpRoot,
        $Context.GalGeneratedXmachineRoot,
        $Context.GalGeneratedProvidersRoot,
        (Get-GalActiveProviderTarget -Provider 'agy')
    )

    foreach ($managedTarget in $managedTargets) {
        if (Test-Path -LiteralPath $managedTarget) {
            return $true
        }

        if ($null -ne (Get-Item -LiteralPath $managedTarget -Force -ErrorAction SilentlyContinue)) {
            return $true
        }
    }

    return $false
}

Write-Host ''
Write-Host '=== GAL install orchestration ==='

if ($Uninstall) {
    $installMode = 'source'
    try {
        $installMode = Get-ConfiguredInstallModeFromContext -Context $script:SetupContext
    }
    catch {
        Write-Host '  [WARN] Could not read machine config during uninstall; falling back to managed-state detection.' -ForegroundColor Yellow
    }

    $ownsManagedUninstall = Test-InstallUninstallOwnership -Context $script:SetupContext
    if ($installMode -ne 'install' -and -not $ownsManagedUninstall -and -not $Purge) {
        Write-Host '  [SKIP] Source-mode uninstall remains owned by the legacy concern scripts.'
        return
    }

    if ($installMode -ne 'install' -and $ownsManagedUninstall) {
        Write-Host '  [OK] Falling back to install-mode uninstall because GAL-managed runtime artifacts are present.'
    }

    Write-Host '  [OK] Install-mode uninstall owns AGY provider-native cleanup.'
    Remove-GalManagedProviderShortcut -Provider 'agy'
    Remove-GalManagedDirectory -Path $script:SetupContext.AgyPluginInstallTarget -Label 'AGY plugin install target'
    Remove-GalManagedDirectory -Path $script:SetupContext.GalStorePluginsRoot -Label 'GAL-managed plugin store'
    Remove-GalManagedDirectory -Path $script:SetupContext.GalGeneratedMcpRoot -Label 'GAL-managed MCP projections'
    Remove-GalManagedDirectory -Path $script:SetupContext.GalGeneratedXmachineRoot -Label 'GAL-managed xmachine projections'
    Remove-GalManagedDirectory -Path $script:SetupContext.GalGeneratedProvidersRoot -Label 'GAL-managed provider projections'

    if ($Purge) {
        if (-not $DryRun -and -not $ConfirmPurge) {
            throw 'Explicit purge requires -ConfirmPurge unless you are running with -DryRun.'
        }

        Write-Host '  [OK] Explicit purge requested; removing preserved machine-local state.'
        Remove-GalManagedDirectory -Path $script:SetupContext.GalConfigRoot -Label 'GAL config root'
        Remove-GalManagedDirectory -Path $script:SetupContext.GalStateDirectory -Label 'GAL state directory'
        Remove-GalManagedDirectory -Path $script:SetupContext.InstallStateFile -Label 'GAL install-state file'
    }

    return
}

$context = $script:SetupContext
$selection = Get-ResolvedRuntimeSelection -SelectedRuntimes $SelectedRuntimes -PrimaryRuntime $PrimaryRuntime -Context $context
$defaultConfig = New-DefaultGalConfig -RepoRoot $RepoRoot -SelectedRuntimes $selection.SelectedRuntimes -PrimaryRuntime $selection.PrimaryRuntime -BootstrapInstall:$BootstrapInstall
$configExists = Test-Path $ConfigPath
$rawConfig = if ($configExists) { Read-JsonOrderedMap $ConfigPath } else { [ordered]@{} }
$effectiveConfig = if ($rawConfig) { Merge-OrderedMap (ConvertTo-OrderedMap $defaultConfig) $rawConfig } else { $defaultConfig }
$effectiveConfig['providerSelections'] = Merge-ProviderSelections -Defaults $defaultConfig['providerSelections'] -Configured $effectiveConfig['providerSelections']

Ensure-SetupDirectories @(
    $context.GalStateRoot,
    $context.GalConfigRoot,
    $context.GalStateDirectory,
    $context.GalStorePluginsRoot,
    $context.GalGeneratedMcpRoot,
    $context.GalGeneratedXmachineRoot,
    $context.GalGeneratedProvidersRoot
)

if (-not $configExists) {
    if ($DryRun) {
        Write-Host ("  [DRY RUN] Would seed machine config: {0} (installMode={1})" -f $ConfigPath, $effectiveConfig['installMode'])
    }
    else {
        Write-JsonOrderedMap $ConfigPath $effectiveConfig
        Write-Host ("  [OK] Seeded machine config: {0}" -f $ConfigPath)
    }
}
else {
    Write-Host ("  [OK] Using machine config: {0}" -f $ConfigPath)
}

$resolverConfigPath = $ConfigPath
$cleanupPaths = [System.Collections.Generic.List[string]]::new()
if ($DryRun -or -not $configExists) {
    $resolverConfigPath = Join-Path $env:TEMP ("gal-config-preview-{0}.json" -f [System.Guid]::NewGuid().ToString('N'))
    Write-JsonOrderedMap $resolverConfigPath $effectiveConfig
    $cleanupPaths.Add($resolverConfigPath)
}

$resolverLockfilePath = if ($DryRun) {
    $tempLockfile = Join-Path $env:TEMP ("gal-lock-preview-{0}.json" -f [System.Guid]::NewGuid().ToString('N'))
    $cleanupPaths.Add($tempLockfile)
    $tempLockfile
}
else {
    $LockfilePath
}

$resolution = & (Join-Path $PSScriptRoot 'Resolve-GalCatalog.ps1') -CatalogPath (Join-Path $RepoRoot 'plugins\catalog.json') -ConfigPath $resolverConfigPath -LockfilePath $resolverLockfilePath -PassThru
if (@($resolution.Errors).Count -gt 0) {
    throw "Catalog resolution failed: $($resolution.Errors -join '; ')"
}

$installMode = [string]$effectiveConfig['installMode']
$defaultProfile = [string]$effectiveConfig['defaultProfile']
$enabledPlugins = @($effectiveConfig['enabledPlugins'])
$primaryProviders = @(Get-ProviderNamesByLane -ProviderSelections $effectiveConfig['providerSelections'] -Lane 'primary' -FallbackSelectedRuntimes $selection.SelectedRuntimes)
$bridgeProviders = @(Get-ProviderNamesByLane -ProviderSelections $effectiveConfig['providerSelections'] -Lane 'bridge' -FallbackSelectedRuntimes $selection.SelectedRuntimes)
$migrationProviders = @(Get-ProviderNamesByLane -ProviderSelections $effectiveConfig['providerSelections'] -Lane 'migration' -FallbackSelectedRuntimes $selection.SelectedRuntimes)

Write-Host ("  [OK] Mode: {0}" -f $installMode)
Write-Host ("  [OK] GAL runtime home: {0}" -f $context.GalStateRoot)
Write-Host ("  [OK] Lockfile target: {0}" -f $LockfilePath)
Write-Host ("  [OK] Active profile: {0}" -f $defaultProfile)
Write-Host ("  [OK] Explicit plugins: {0}" -f (Get-DisplayValue -Items $enabledPlugins))
Write-Host ("  [OK] Resolved plugins: {0}" -f (Get-DisplayValue -Items @($resolution.ResolvedPlugins | ForEach-Object { $_.pluginId })))
Write-Host ("  [OK] Primary provider lanes: {0}" -f (Get-DisplayValue -Items $primaryProviders))
Write-Host ("  [OK] Bridge lanes: {0}" -f (Get-DisplayValue -Items $bridgeProviders))
Write-Host ("  [OK] Migration lanes: {0}" -f (Get-DisplayValue -Items $migrationProviders))

if ($installMode -eq 'source') {
    Write-Host ("  [OK] Source mode galRoot: {0}" -f [string]$effectiveConfig['galRoot'])
    Write-Host ("  [OK] Source mode devMode: {0}" -f (ConvertTo-Bool $effectiveConfig['devMode'] $true))
    Write-Host ("  [OK] Local override boundary: explicit machine-local bindings via {0}" -f $context.GalXmachineConfigFile)
    Write-Host '  [OK] Repo-root links and local overrides stay source-mode-only contributor paths.'
}
else {
    Write-Host ("  [OK] Install mode projections root: {0}" -f $context.GalGeneratedRoot)
    Write-Host '  [OK] Install mode disables repo-root links and source-only local overrides.'
    if ($BootstrapInstall -and -not $configExists) {
        Write-Host '  [OK] First launch bootstrap path seeded install mode because no machine config existed yet.'
    }
    if ($primaryProviders.Count -gt 0) {
        & (Join-Path $PSScriptRoot 'Build-ProviderPlugins.ps1') -RepoRoot $RepoRoot -ConfigPath $resolverConfigPath -LockfilePath $resolverLockfilePath -Providers $primaryProviders -DryRun:$DryRun -Force:$Force
    }
    else {
        Write-Host '  [SKIP] No primary providers selected for provider-native install orchestration.'
    }

    if ($bridgeProviders.Count -gt 0) {
        Write-Host ("  [OK] Bridge lanes stay capability-only and target ~/.gal/active/<provider>: {0}" -f (Get-DisplayValue -Items $bridgeProviders))
    }
    if ($migrationProviders.Count -gt 0) {
        Write-Host ("  [OK] Migration lanes stay compatibility-only: {0}" -f (Get-DisplayValue -Items $migrationProviders))
    }
}

foreach ($path in $cleanupPaths) {
    if (Test-Path $path) {
        Remove-Item -LiteralPath $path -Force
    }
}