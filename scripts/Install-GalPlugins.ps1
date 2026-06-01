#Requires -Version 5.1

[CmdletBinding()]
param(
    [string]$RepoRoot,
    [string]$ConfigPath = (Join-Path $env:USERPROFILE '.gal\config\config.json'),
    [string]$LockfilePath = (Join-Path $env:USERPROFILE '.gal\state\plugins.lock.json'),
    [string[]]$SelectedRuntimes,
    [string]$PrimaryRuntime,
    [Alias('Doctor')][switch]$Check,
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

if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
    $RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
}

. (Join-Path (Join-Path $PSScriptRoot 'common') 'Common.ps1')
. (Join-Path (Join-Path $PSScriptRoot 'common') 'ProviderPlugin.ps1')

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

    if ($providerNames.Count -eq 0 -and @($FallbackSelectedRuntimes).Count -gt 0) {
        foreach ($runtime in @($FallbackSelectedRuntimes)) {
            if ((Get-ProviderLaneForRuntime $runtime) -ne $Lane) {
                continue
            }

            $providerName = Get-ProviderFromRuntime $runtime
            if ($providerNames -notcontains $providerName) {
                $providerNames.Add($providerName)
            }
        }
    }

    return [string[]]$providerNames.ToArray()
}

function Get-ProviderNamesFromSelectedRuntimesByLane {
    param(
        [string[]]$SelectedRuntimes,
        [string]$Lane
    )

    $providerNames = [System.Collections.Generic.List[string]]::new()
    foreach ($runtime in @($SelectedRuntimes)) {
        if ([string]::IsNullOrWhiteSpace($runtime)) {
            continue
        }

        if ((Get-ProviderLaneForRuntime $runtime) -ne $Lane) {
            continue
        }

        $providerName = Get-ProviderFromRuntime $runtime
        if ($providerNames -notcontains $providerName) {
            $providerNames.Add($providerName)
        }
    }

    return [string[]]$providerNames.ToArray()
}

function Get-DisplayValue([string[]]$Items, [string]$Fallback = 'none') {
    if ($null -eq $Items -or @($Items).Count -eq 0) {
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
        $Context.CopilotPluginInstallTarget,
        $Context.GalRootCopilot,
        (Join-Path $Context.GalGeneratedProvidersRoot 'codex\managed.json'),
        (Join-Path $Context.GalPluginsRoot '.agents\plugins\marketplace.json'),
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

function Get-ClaudeLifecycleStatePath {
    param(
        [pscustomobject]$Context
    )

    return Join-Path $Context.GalGeneratedProvidersRoot 'claude\managed.json'
}

function Get-ClaudeMarketplaceManifestPath {
    param([pscustomobject]$Context)
    return Join-Path $Context.GalPluginsRoot '.claude-plugin\marketplace.json'
}

function Get-CodexLifecycleStatePath {
    param(
        [pscustomobject]$Context
    )

    return Join-Path $Context.GalGeneratedProvidersRoot 'codex\managed.json'
}

function Get-CodexMarketplaceRoot {
    param([pscustomobject]$Context)

    return $Context.GalPluginsRoot
}

function Get-CodexMarketplaceManifestPath {
    param([pscustomobject]$Context)

    return Join-Path (Get-CodexMarketplaceRoot -Context $Context) '.agents\plugins\marketplace.json'
}

function Ensure-CodexMarketplaceManifest {
    param([pscustomobject]$Context)

    $manifestPath = Get-CodexMarketplaceManifestPath -Context $Context
    $manifestDir = Split-Path -Parent $manifestPath
    Ensure-SetupDirectories @($manifestDir)

    $marketplace = [ordered]@{
        name = 'gal-marketplace'
        interface = [ordered]@{ displayName = 'GAL Plugin Marketplace' }
        plugins = @(
            [ordered]@{
                name = 'gal'
                source = [ordered]@{ source = 'local'; path = './gal' }
                policy = [ordered]@{ installation = 'AVAILABLE'; authentication = 'ON_INSTALL' }
                category = 'Engineering'
            }
        )
    }

    if ($DryRun) {
        Write-Host ("  [DRY RUN] Would refresh Codex marketplace descriptor: {0}" -f $manifestPath)
        return $manifestPath
    }

    $marketplace | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $manifestPath -Encoding utf8
    Write-Host ("  [OK] Refreshed Codex marketplace descriptor: {0}" -f $manifestPath)
    return $manifestPath
}

function Ensure-ClaudeMarketplaceManifest {
    param([pscustomobject]$Context)

    $manifestPath = Get-ClaudeMarketplaceManifestPath -Context $Context
    $manifestDir = Split-Path $manifestPath -Parent

    $manifest = [ordered]@{
        name = 'gal'
        owner = [ordered]@{ name = 'GAL' }
        description = 'Golem Agents Legion — document-driven AI working system'
        plugins = @(
            [ordered]@{
                name = 'gal'
                source = './gal'
                description = 'Golem Agents Legion plugin for Claude Code'
            }
        )
    }

    if ($DryRun) {
        Write-Host ("  [DRY RUN] Would write marketplace manifest: {0}" -f $manifestPath)
        return
    }

    if (-not (Test-Path $manifestDir)) {
        New-Item -ItemType Directory -Path $manifestDir -Force | Out-Null
    }

    Write-JsonOrderedMap $manifestPath $manifest
    Write-Host ("  [OK] Wrote marketplace manifest: {0}" -f $manifestPath)
}

function Install-ClaudePluginViaMarketplace {
    param(
        [pscustomobject]$Context,
        [bool]$Replace = $false
    )

    $marketplaceRoot = $Context.GalPluginsRoot
    $pluginName = 'gal'
    $marketplaceName = 'gal'

    Ensure-ClaudeMarketplaceManifest -Context $Context

    if ($DryRun) {
        Write-Host ("  [DRY RUN] Would add GAL marketplace: claude plugin marketplace add --scope user `"{0}`"" -f $marketplaceRoot)
        Write-Host ("  [DRY RUN] Would install plugin: claude plugin install {0} --scope user" -f $pluginName)
        return $true
    }

    $listOutput = (& claude plugin list 2>&1 | Out-String)
    $alreadyInstalled = ($LASTEXITCODE -eq 0 -and ($listOutput -match ('(?m)^\s*{0}\b' -f [regex]::Escape($pluginName))))

    if ($alreadyInstalled) {
        if ($Replace) {
            Write-Host ("  [OK] Replacing existing Claude plugin '{0}'." -f $pluginName)
            & claude plugin uninstall $pluginName --scope user 2>&1 | Out-Null
        }
        else {
            Write-Host ("  [OK] Claude plugin '{0}' already installed via marketplace." -f $pluginName)
            return $true
        }
    }

    $addOutput = (& claude plugin marketplace add --scope user $marketplaceRoot 2>&1 | Out-String)
    if ($LASTEXITCODE -ne 0) {
        Write-Host ("  [WARN] Failed to add GAL marketplace from '{0}': {1}" -f $marketplaceRoot, $addOutput.Trim()) -ForegroundColor Yellow
        return $false
    }
    Write-Host ("  [OK] Added GAL marketplace from: {0}" -f $marketplaceRoot)

    $installOutput = (& claude plugin install $pluginName --scope user 2>&1 | Out-String)
    if ($LASTEXITCODE -ne 0) {
        Write-Host ("  [WARN] Failed to install Claude plugin '{0}': {1}" -f $pluginName, $installOutput.Trim()) -ForegroundColor Yellow
        return $false
    }
    Write-Host ("  [OK] Installed Claude plugin '{0}' via marketplace '{1}'." -f $pluginName, $marketplaceName)
    return $true
}

function Write-ClaudeLifecycleState {
    param(
        [pscustomobject]$Context,
        [System.Collections.IDictionary]$State
    )

    $statePath = Get-ProviderManagedStatePath -Provider 'claude' -Context $Context
    if ($DryRun) {
        Write-Host ("  [DRY RUN] Would write Claude lifecycle state: {0}" -f $statePath)
        return
    }

    Write-JsonOrderedMap $statePath $State
    Write-Host ("  [OK] Wrote Claude lifecycle state: {0}" -f $statePath)
}

function Write-CopilotLifecycleState {
    param(
        [pscustomobject]$Context,
        [System.Collections.IDictionary]$State
    )

    $statePath = Get-ProviderManagedStatePath -Provider 'copilot' -Context $Context
    if ($DryRun) {
        Write-Host ("  [DRY RUN] Would write Copilot lifecycle state: {0}" -f $statePath)
        return
    }

    Write-JsonOrderedMap $statePath $State
    Write-Host ("  [OK] Wrote Copilot lifecycle state: {0}" -f $statePath)
}

function Write-CodexLifecycleState {
    param(
        [pscustomobject]$Context,
        [System.Collections.IDictionary]$State
    )

    $statePath = Get-CodexLifecycleStatePath -Context $Context
    if ($DryRun) {
        Write-Host ("  [DRY RUN] Would write Codex lifecycle state: {0}" -f $statePath)
        return
    }

    Write-JsonOrderedMap $statePath $State
    Write-Host ("  [OK] Wrote Codex lifecycle state: {0}" -f $statePath)
}

function Update-CopilotManifestVersion {
    param(
        [Parameter(Mandatory)]
        [string]$ManifestPath
    )

    if (-not (Test-Path $ManifestPath)) {
        return $null
    }

    $manifest = Read-JsonOrderedMap $ManifestPath
    if (-not $manifest) {
        return $null
    }

    $currentVersion = [string]$manifest['version']
    if ([string]::IsNullOrWhiteSpace($currentVersion)) {
        $currentVersion = '1.0.0'
    }

    $bumpedVersion = '{0}.host{1}' -f $currentVersion, (Get-Date -Format 'yyyyMMddHHmmss')
    $manifest['version'] = $bumpedVersion
    Write-JsonOrderedMap $ManifestPath $manifest
    return $bumpedVersion
}

function Sync-CopilotPluginProjection {
    param(
        [string]$CanonicalRoot,
        [pscustomobject]$Context
    )

    Ensure-SetupDirectories @((Split-Path $Context.CopilotPluginInstallTarget -Parent))

    if (-not (Test-Path $CanonicalRoot)) {
        throw "Copilot canonical root not found for projection: $CanonicalRoot"
    }

    Remove-SafeLink $Context.GalRootCopilot

    if (New-SafeSymlink $Context.CopilotPluginInstallTarget $CanonicalRoot 'Directory') {
        return [pscustomobject]@{
            projectionRoot = $Context.CopilotPluginInstallTarget
            refreshedCopyToHost = $false
            versionBumpedTo = $null
        }
    }

    Write-Host '  [WARN] Copilot projection link was unavailable; refreshing host copy instead.' -ForegroundColor Yellow

    if ($DryRun) {
        Write-Host ("  [DRY RUN] Would refresh Copilot host copy: {0}" -f $Context.CopilotPluginInstallTarget)
        Write-Host ("  [DRY RUN] Would bump Copilot manifest version in host copy: {0}" -f (Join-Path $Context.CopilotPluginInstallTarget 'copilot-manifest.json'))
        return [pscustomobject]@{
            projectionRoot = $Context.CopilotPluginInstallTarget
            refreshedCopyToHost = $true
            versionBumpedTo = 'preview-host-bump'
        }
    }

    if (Test-Path $Context.CopilotPluginInstallTarget) {
        Remove-Item -LiteralPath $Context.CopilotPluginInstallTarget -Recurse -Force
    }

    Copy-Item -LiteralPath $CanonicalRoot -Destination $Context.CopilotPluginInstallTarget -Recurse -Force
    $bumpedVersion = Update-CopilotManifestVersion -ManifestPath (Join-Path $Context.CopilotPluginInstallTarget 'copilot-manifest.json')

    return [pscustomobject]@{
        projectionRoot = $Context.CopilotPluginInstallTarget
        refreshedCopyToHost = $true
        versionBumpedTo = $bumpedVersion
    }
}

function Sync-ClaudePluginProjection {
    param(
        [string]$CanonicalRoot,
        [pscustomobject]$Context
    )

    Ensure-SetupDirectories @($Context.ClaudeSkillsRoot)

    if (-not (Test-Path $CanonicalRoot)) {
        throw "Claude canonical root not found for projection: $CanonicalRoot"
    }

    Remove-SafeLink $Context.ClaudeLegacyPluginInstallTarget

    if (-not (New-SafeSymlink $Context.ClaudePluginInstallTarget $CanonicalRoot 'Directory')) {
        throw "Failed to project Claude plugin into $($Context.ClaudePluginInstallTarget)"
    }

    return $Context.ClaudePluginInstallTarget
}

function Invoke-ClaudePluginLifecycle {
    param(
        [string]$RepoRoot,
        [pscustomobject]$Context
    )

    if (-not $Context.InstallClaude) {
        return
    }

    Write-Host '  [OK] Evaluating Claude plugin lifecycle.'
    $canonicalRoot = Get-GalPluginRoot -PluginId 'gal'
    $packageOutputRoot = Get-ClaudePluginPackageOutputRoot -RepoRoot $RepoRoot
    $manifestPath = Get-ClaudePluginManifestPath -PluginRoot $canonicalRoot
    $contract = Get-ClaudePluginInstallContract
    $support = Get-ClaudeCliLifecycleSupport

    $state = [ordered]@{
        schemaVersion = 1
        provider = 'claude'
        canonicalRoot = $canonicalRoot
        packageOutputRoot = $packageOutputRoot
        projectionRoot = $Context.ClaudePluginInstallTarget
        installTarget = $Context.ClaudePluginInstallTarget
        manifestPath = $manifestPath
        generatedAt = (Get-Date -Format 'o')
        status = 'linked-projection'
        readSurface = (Resolve-ProviderManagedReadSurface -Status 'linked-projection')
        cli = [ordered]@{
            available = [bool]$support.cliAvailable
            validateSupported = [bool]$support.validateSupported
            localArtifactInstallSupported = [bool]$support.localArtifactInstallSupported
            installScopeSupported = [bool]$support.installScopeSupported
            installHelpSummary = $support.installHelpSummary
        }
        validation = [ordered]@{
            command = $contract.validationCommand
            strictPassed = $false
        }
        lifecycle = [ordered]@{
            mode = 'artifact-only'
            stagedPluginRoot = $null
            marketplaceRoot = $null
            marketplaceName = 'gal'
            sessionLoadCommand = ("claude --plugin-dir `"{0}`"" -f $canonicalRoot)
            installCommandTemplate = 'claude plugin install <plugin> --scope <scope>'
            updateCommandTemplate = 'claude plugin update <plugin> --scope <scope>'
            uninstallCommandTemplate = 'claude plugin uninstall <plugin> --scope <scope>'
        }
        notes = @(
            'Artifact validation is supported when the local Claude CLI exposes `claude plugin validate <path>`.',
            'Persistent install projects ~/.claude/skills/gal to the canonical GAL plugin root.',
            '~/.claude/plugins/gal is treated as a legacy location and is removed when GAL refreshes the Claude projection.'
        )
    }

    if (-not (Test-Path $canonicalRoot)) {
        throw "Claude canonical root not found: $canonicalRoot"
    }

    if (-not (Test-Path $manifestPath)) {
        throw "Claude manifest not found: $manifestPath"
    }

    if (-not $support.cliAvailable) {
        Write-Host '  [WARN] Claude CLI not found on PATH; artifact is built but lifecycle validation is unavailable.' -ForegroundColor Yellow
        $state['lifecycle']['stagedPluginRoot'] = Sync-ClaudePluginProjection -CanonicalRoot $canonicalRoot -Context $Context
        Write-Host ("  [OK] Projected Claude skills root: {0}" -f $state['lifecycle']['stagedPluginRoot'])
        Write-ClaudeLifecycleState -Context $Context -State $state
        return
    }

    if (-not $support.validateSupported) {
        Write-Host '  [WARN] Claude CLI is present but `claude plugin validate` is unavailable; recording artifact-only lifecycle status.' -ForegroundColor Yellow
        $state['lifecycle']['stagedPluginRoot'] = Sync-ClaudePluginProjection -CanonicalRoot $canonicalRoot -Context $Context
        Write-Host ("  [OK] Projected Claude skills root: {0}" -f $state['lifecycle']['stagedPluginRoot'])
        Write-ClaudeLifecycleState -Context $Context -State $state
        return
    }

    if ($DryRun) {
        $state['lifecycle']['stagedPluginRoot'] = $Context.ClaudePluginInstallTarget
        Write-Host ("  [DRY RUN] Would project Claude skills root into: {0}" -f $Context.ClaudePluginInstallTarget)
        Write-Host ("  [DRY RUN] Would run Claude plugin validation: claude plugin validate --strict `"{0}`"" -f $canonicalRoot)
        Write-ClaudeLifecycleState -Context $Context -State $state
        return
    }

    $validationOutput = (& claude plugin validate --strict $canonicalRoot 2>&1 | Out-String)
    if ($LASTEXITCODE -ne 0) {
        throw "Claude plugin validation failed for $canonicalRoot`n$validationOutput"
    }

    $state['validation']['strictPassed'] = $true
    $state['lifecycle']['mode'] = 'session-load-only'
    Write-Host '  [OK] Claude plugin validation passed.'
    $state['lifecycle']['stagedPluginRoot'] = Sync-ClaudePluginProjection -CanonicalRoot $canonicalRoot -Context $Context
    Write-Host ("  [OK] Projected Claude skills root: {0}" -f $state['lifecycle']['stagedPluginRoot'])

    if ($support.localArtifactInstallSupported) {
        Write-Host '  [OK] Claude CLI reports local artifact install support.'
    }

    Write-ClaudeLifecycleState -Context $Context -State $state
}

function Invoke-CopilotPluginLifecycle {
    param(
        [string]$RepoRoot,
        [pscustomobject]$Context
    )

    if (-not $Context.InstallCopilot) {
        return
    }

    Write-Host '  [OK] Evaluating Copilot plugin lifecycle.'
    $canonicalRoot = Get-GalPluginRoot -PluginId 'gal'
    $packageOutputRoot = Get-CopilotPluginPackageOutputRoot -RepoRoot $RepoRoot
    $manifestPath = Get-CopilotPluginManifestPath -PluginRoot $canonicalRoot
    $contract = Get-CopilotPluginInstallContract
    $support = Get-CopilotCliLifecycleSupport

    if (-not (Test-Path $canonicalRoot) -or -not (Test-Path $manifestPath)) {
        Write-Host '  [WARN] Copilot canonical artifact was missing; rerendering shared plugin root before lifecycle projection.' -ForegroundColor Yellow
        & (Join-Path $PSScriptRoot 'Build-CorePlugin.ps1') -RepoRoot $RepoRoot -Force:$Force | Out-Null
    }

    $state = [ordered]@{
        schemaVersion = 1
        provider = 'copilot'
        canonicalRoot = $canonicalRoot
        packageOutputRoot = $packageOutputRoot
        projectionRoot = $Context.CopilotPluginInstallTarget
        installTarget = $Context.CopilotPluginInstallTarget
        manifestPath = $manifestPath
        generatedAt = (Get-Date -Format 'o')
        status = 'linked-projection'
        readSurface = (Resolve-ProviderManagedReadSurface -Status 'linked-projection')
        cli = [ordered]@{
            available = [bool]$support.cliAvailable
            validateSupported = [bool]$support.validateSupported
            localArtifactInstallSupported = [bool]$support.localArtifactInstallSupported
            marketplaceInstallSupported = [bool]$support.marketplaceInstallSupported
            installScopeSupported = [bool]$support.installScopeSupported
            installHelpSummary = $support.installHelpSummary
        }
        validation = [ordered]@{
            command = $contract.validationCommand
            strictPassed = $false
        }
        lifecycle = [ordered]@{
            mode = [string]$support.installMode
            stagedPluginRoot = $null
            refreshedCopyToHost = $false
            versionBumpedTo = $null
            sessionLoadCommand = $contract.developmentLoadCommand
            installCommandTemplate = $contract.lifecycleCommands[0]
            updateCommandTemplate = $contract.lifecycleCommands[1]
            uninstallCommandTemplate = $contract.lifecycleCommands[2]
        }
        notes = @(
            'Copilot CLI probing is best-effort; missing `gh` only downgrades lifecycle metadata, not artifact projection.',
            'Persistent install prefers a link projection at ~/.copilot/installed-plugins/gal-copilot/gal so VS Code Copilot and Copilot CLI read the same payload.',
            '~/.copilot/gal is treated as a legacy GAL_ROOT link and is removed when GAL refreshes the Copilot projection.',
            'When link projection is unavailable, GAL refreshes a host copy and bumps copilot-manifest.json version to avoid stale Copilot cache reuse.'
        )
    }

    if (-not (Test-Path $canonicalRoot)) {
        throw "Copilot canonical root not found: $canonicalRoot"
    }

    if (-not (Test-Path $manifestPath)) {
        throw "Copilot manifest not found: $manifestPath"
    }

    if (-not $support.cliAvailable) {
        Write-Host '  [WARN] GitHub CLI not found on PATH; projecting Copilot plugin without CLI lifecycle validation.' -ForegroundColor Yellow
    }
    elseif (-not $support.localArtifactInstallSupported) {
        Write-Host '  [WARN] GitHub CLI is present but local Copilot artifact install support was not detected; keeping projection-managed lifecycle.' -ForegroundColor Yellow
    }
    else {
        Write-Host '  [OK] GitHub CLI reports local Copilot artifact install support.'
    }

    $projection = Sync-CopilotPluginProjection -CanonicalRoot $canonicalRoot -Context $Context
    $state['projectionRoot'] = $projection.projectionRoot
    $state['installTarget'] = $projection.projectionRoot
    $state['lifecycle']['stagedPluginRoot'] = $projection.projectionRoot
    $state['lifecycle']['refreshedCopyToHost'] = [bool]$projection.refreshedCopyToHost
    $state['lifecycle']['versionBumpedTo'] = $projection.versionBumpedTo

    if ($projection.refreshedCopyToHost) {
        $state['status'] = Resolve-ProviderManagedStateStatus -Mode 'native-install' -LifecycleStatus 'host-copy-refreshed' -ProjectionRoot $null -RefreshedCopyToHost $true
        $state['readSurface'] = Resolve-ProviderManagedReadSurface -Status $state['status']
        Write-Host ("  [OK] Refreshed Copilot host copy: {0}" -f $projection.projectionRoot)
    }
    else {
        Write-Host ("  [OK] Projected Copilot plugin root: {0}" -f $projection.projectionRoot)
    }

    Write-CopilotLifecycleState -Context $Context -State $state
}

function Invoke-CodexPluginLifecycle {
    param(
        [string]$RepoRoot,
        [pscustomobject]$Context
    )

    if (-not $Context.InstallCodex) {
        return
    }

    Write-Host '  [OK] Evaluating Codex plugin lifecycle.'
    $canonicalRoot = Get-GalPluginRoot -PluginId 'gal'
    $packageOutputRoot = Get-CodexPluginPackageOutputRoot -RepoRoot $RepoRoot
    $manifestPath = Get-CodexPluginManifestPath -PluginRoot $canonicalRoot
    $contract = Get-CodexPluginInstallContract
    $support = Get-CodexCliLifecycleSupport
    $marketplaceRoot = Get-CodexMarketplaceRoot -Context $Context
    $marketplaceName = 'gal-marketplace'
    $pluginSelector = 'gal@gal-marketplace'

    if (-not (Test-Path $canonicalRoot) -or -not (Test-Path $manifestPath)) {
        Write-Host '  [WARN] Codex canonical artifact was missing; rerendering shared plugin root before marketplace registration.' -ForegroundColor Yellow
        & (Join-Path $PSScriptRoot 'Build-CorePlugin.ps1') -RepoRoot $RepoRoot -Force:$Force | Out-Null
    }

    $marketplaceManifestPath = Ensure-CodexMarketplaceManifest -Context $Context
    $status = 'unprojected-artifact'

    $state = [ordered]@{
        schemaVersion = 1
        provider = 'codex'
        canonicalRoot = $canonicalRoot
        packageOutputRoot = $packageOutputRoot
        projectionRoot = $null
        installTarget = $pluginSelector
        manifestPath = $manifestPath
        generatedAt = (Get-Date -Format 'o')
        status = $status
        readSurface = (Resolve-ProviderManagedReadSurface -Status $status)
        cli = [ordered]@{
            available = [bool]$support.cliAvailable
            validateSupported = [bool]$support.validateSupported
            localArtifactInstallSupported = [bool]$support.localArtifactInstallSupported
            marketplaceInstallSupported = [bool]$support.marketplaceInstallSupported
            installScopeSupported = [bool]$support.installScopeSupported
            installHelpSummary = $support.installHelpSummary
        }
        validation = [ordered]@{
            command = $contract.validationCommand
            strictPassed = $false
        }
        lifecycle = [ordered]@{
            mode = [string]$support.installMode
            stagedPluginRoot = $canonicalRoot
            marketplaceRoot = $marketplaceRoot
            marketplaceManifestPath = $marketplaceManifestPath
            marketplaceName = $marketplaceName
            installedSelector = $pluginSelector
            sessionLoadCommand = $contract.developmentLoadCommand
            installCommandTemplate = $contract.lifecycleCommands[0]
            updateCommandTemplate = $contract.lifecycleCommands[1]
            uninstallCommandTemplate = $contract.lifecycleCommands[2]
            pluginRemovedBeforeAdd = $false
        }
        notes = @(
            'Codex lifecycle is independent from AGY and is routed through Install-GalPlugins.ps1.',
            'GAL refreshes the local marketplace descriptor before each Codex registration so codex plugin marketplace add reads the canonical plugin root.',
            'Codex updates are delivered by marketplace-copy semantics; the canonical .codex-plugin/plugin.json version must change on each render to satisfy version-gated refreshes.'
        )
    }

    if (-not (Test-Path $canonicalRoot)) {
        throw "Codex canonical root not found: $canonicalRoot"
    }

    if (-not (Test-Path $manifestPath)) {
        throw "Codex manifest not found: $manifestPath"
    }

    if (-not $support.cliAvailable) {
        Write-Host '  [WARN] Codex CLI not found on PATH; refreshed marketplace descriptor only and recorded artifact-only lifecycle state.' -ForegroundColor Yellow
        $state['lifecycle']['mode'] = 'artifact-only'
        Write-CodexLifecycleState -Context $Context -State $state
        return
    }

    if (-not $support.marketplaceInstallSupported) {
        Write-Host '  [WARN] Codex CLI is present but marketplace install support was not detected; recording artifact-only lifecycle state.' -ForegroundColor Yellow
        $state['lifecycle']['mode'] = 'artifact-only'
        Write-CodexLifecycleState -Context $Context -State $state
        return
    }

    if ($DryRun) {
        Write-Host ("  [DRY RUN] Would register Codex marketplace: codex plugin marketplace add `"{0}`"" -f $marketplaceRoot)
        Write-Host ("  [DRY RUN] Would refresh Codex plugin install: codex plugin remove {0} ; codex plugin add {0}" -f $pluginSelector)
        Write-CodexLifecycleState -Context $Context -State $state
        return
    }

    $marketplaceAddOutput = (& codex plugin marketplace add $marketplaceRoot 2>&1 | Out-String)
    if ($LASTEXITCODE -ne 0) {
        Write-Host '  [WARN] Codex marketplace add returned non-zero; attempting marketplace re-registration.' -ForegroundColor Yellow
        & codex plugin marketplace remove $marketplaceName 2>&1 | Out-Null
        $marketplaceAddOutput = (& codex plugin marketplace add $marketplaceRoot 2>&1 | Out-String)
        if ($LASTEXITCODE -ne 0) {
            throw "Codex marketplace registration failed for $marketplaceRoot`n$marketplaceAddOutput"
        }
    }
    Write-Host ("  [OK] Registered Codex marketplace '{0}'." -f $marketplaceName)

    $pluginRemoveOutput = (& codex plugin remove $pluginSelector 2>&1 | Out-String)
    if ($LASTEXITCODE -eq 0) {
        $state['lifecycle']['pluginRemovedBeforeAdd'] = $true
        Write-Host ("  [OK] Removed existing Codex plugin '{0}' before refresh." -f $pluginSelector)
    }

    $pluginAddOutput = (& codex plugin add $pluginSelector 2>&1 | Out-String)
    if ($LASTEXITCODE -ne 0) {
        throw "Codex plugin install failed for $pluginSelector`n$pluginAddOutput"
    }

    Write-Host ("  [OK] Installed Codex plugin '{0}'." -f $pluginSelector)
    Write-CodexLifecycleState -Context $Context -State $state
}

function Test-PathWithinRoot {
    param(
        [string]$Path,
        [string]$Root
    )

    if ([string]::IsNullOrWhiteSpace($Path) -or [string]::IsNullOrWhiteSpace($Root)) {
        return $false
    }

    $normalizedPath = [System.IO.Path]::GetFullPath($Path).TrimEnd('\')
    $normalizedRoot = [System.IO.Path]::GetFullPath($Root).TrimEnd('\')
    return $normalizedPath.Equals($normalizedRoot, [System.StringComparison]::OrdinalIgnoreCase) -or $normalizedPath.StartsWith($normalizedRoot + '\', [System.StringComparison]::OrdinalIgnoreCase)
}

function Get-CopilotDoctorUnknownItems {
    param([pscustomobject]$Context)

    if (-not (Test-Path $Context.CopilotRoot)) {
        return @()
    }

    $knownRoots = @(
        $Context.CopilotPluginInstallTarget,
        (Split-Path $Context.CopilotPluginInstallTarget -Parent),
        $Context.GalRootCopilot,
        $Context.AgentsTarget,
        $Context.SkillsTarget
    ) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }

    $unknownItems = New-Object System.Collections.Generic.List[string]
    foreach ($item in (Get-ChildItem -LiteralPath $Context.CopilotRoot -Force -Recurse -ErrorAction SilentlyContinue)) {
        $isKnown = $false
        foreach ($knownRoot in $knownRoots) {
            if (Test-PathWithinRoot -Path $item.FullName -Root $knownRoot) {
                $isKnown = $true
                break
            }
        }

        if (-not $isKnown) {
            $unknownItems.Add($item.FullName)
        }
    }

    return @($unknownItems | Sort-Object -Unique)
}

function Invoke-GalProviderDoctor {
    param(
        [pscustomobject]$Context,
        [string[]]$SelectedRuntimes
    )

    $canonicalRoot = Get-GalPluginRoot -PluginId 'gal'
    $copilotSelected = @($SelectedRuntimes) -contains 'copilot'
    $canonicalManifestPath = Join-Path $canonicalRoot 'copilot-manifest.json'
    $copilotHostManifestPath = Join-Path $Context.CopilotPluginInstallTarget 'copilot-manifest.json'

    Write-Host ''
    Write-Host '=== GAL provider doctor ==='
    Write-Host 'CANONICAL:'
    if (Test-Path $canonicalRoot) {
        Write-Host ("  [OK] {0} — canonical GAL plugin root present" -f $canonicalRoot)
    }
    else {
        Write-Host ("  [WARN] {0} — canonical GAL plugin root missing" -f $canonicalRoot)
    }

    Write-Host 'EXPECTED PROJECTION:'
    if ($copilotSelected) {
        Write-Host ("  [INFO] {0} — expected Copilot plugin projection root" -f $Context.CopilotPluginInstallTarget)
    }
    else {
        Write-Host '  [INFO] No Copilot provider selected for this doctor invocation.'
    }

    Write-Host 'HOST-MANAGED:'
    if ($copilotSelected -and (Test-Path $Context.CopilotPluginInstallTarget) -and -not (Test-SymlinkOrJunction $Context.CopilotPluginInstallTarget)) {
        Write-Host ("  [WARN] {0} — Copilot host-managed copy detected" -f $Context.CopilotPluginInstallTarget)
    }
    else {
        Write-Host '  [INFO] No Copilot host-managed copies detected.'
    }

    Write-Host 'LEGACY GAL:'
    if (Test-GalRepoLink -Path $Context.GalRootCopilot -TargetFragment $Context.GalStateRoot) {
        Write-Host ("  [WARN] {0} — legacy Copilot GAL_ROOT link" -f $Context.GalRootCopilot)
    }
    else {
        Write-Host '  [INFO] No legacy Copilot GAL_ROOT link detected.'
    }

    Write-Host 'USER-OWNED-UNKNOWN:'
    $unknownItems = @(Get-CopilotDoctorUnknownItems -Context $Context)
    if ($unknownItems.Count -gt 0) {
        foreach ($unknownItem in $unknownItems) {
            Write-Host ("  [INFO] {0} — user-owned or unknown Copilot artifact" -f $unknownItem)
        }
    }
    else {
        Write-Host '  [INFO] No user-owned unknown Copilot artifacts detected.'
    }

    Write-Host 'STALE:'
    if ($copilotSelected -and (Test-Path $Context.CopilotPluginInstallTarget) -and -not (Test-SymlinkOrJunction $Context.CopilotPluginInstallTarget) -and (Test-Path $canonicalManifestPath) -and (Test-Path $copilotHostManifestPath)) {
        $canonicalManifest = Read-JsonOrderedMap $canonicalManifestPath
        $hostManifest = Read-JsonOrderedMap $copilotHostManifestPath
        $canonicalVersion = if ($canonicalManifest -and $canonicalManifest.Contains('version')) { [string]$canonicalManifest['version'] } else { '' }
        $hostVersion = if ($hostManifest -and $hostManifest.Contains('version')) { [string]$hostManifest['version'] } else { '' }

        if (-not [string]::Equals($canonicalVersion, $hostVersion, [System.StringComparison]::Ordinal)) {
            Write-Host ("  [WARN] {0} — host copy diverges from canonical (canonical={1}; host={2})" -f $Context.CopilotPluginInstallTarget, $canonicalVersion, $hostVersion)
        }
        else {
            Write-Host ("  [OK] {0} — host copy version matches canonical" -f $Context.CopilotPluginInstallTarget)
        }
    }
    else {
        Write-Host '  [INFO] No stale host-copy findings detected.'
    }
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

        Write-Host '  [OK] Install-mode uninstall owns AGY, Copilot, Codex, and Claude provider-lifecycle metadata cleanup.'
        if ($DryRun) {
            Write-Host '  [DRY RUN] Would remove Codex plugin: codex plugin remove gal@gal-marketplace'
            Write-Host '  [DRY RUN] Would unregister Codex marketplace: codex plugin marketplace remove gal-marketplace'
        }
        elseif (Test-CommandAvailable 'codex') {
            & codex plugin remove 'gal@gal-marketplace' 2>&1 | Out-Null
            & codex plugin marketplace remove 'gal-marketplace' 2>&1 | Out-Null
        }
        else {
            Write-Host '  [WARN] Codex CLI not found on PATH; skipping Codex plugin unregister/remove during uninstall.' -ForegroundColor Yellow
        }

    Remove-GalManagedProviderShortcut -Provider 'agy'
    Remove-GalManagedDirectory -Path $script:SetupContext.AgyPluginInstallTarget -Label 'AGY plugin install target'

    if (Test-SymlinkOrJunction $script:SetupContext.CopilotPluginInstallTarget) {
        Remove-SafeLink $script:SetupContext.CopilotPluginInstallTarget
    }
    else {
        Remove-GalManagedDirectory -Path $script:SetupContext.CopilotPluginInstallTarget -Label 'Copilot plugin install target'
    }
    Remove-SafeLink $script:SetupContext.GalRootCopilot
    Remove-SafeLink $script:SetupContext.ClaudePluginInstallTarget
    Remove-SafeLink $script:SetupContext.ClaudeLegacyPluginInstallTarget
    Remove-GalManagedDirectory -Path $script:SetupContext.GalPluginsRoot -Label 'GAL canonical plugin root'
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

if ($Check) {
    Invoke-GalProviderDoctor -Context $context -SelectedRuntimes $selection.SelectedRuntimes
    return
}

$defaultConfig = New-DefaultGalConfig -RepoRoot $RepoRoot -SelectedRuntimes $selection.SelectedRuntimes -PrimaryRuntime $selection.PrimaryRuntime -BootstrapInstall:$BootstrapInstall
$configExists = Test-Path $ConfigPath
$rawConfig = if ($configExists) { Read-JsonOrderedMap $ConfigPath } else { [ordered]@{} }
$effectiveConfig = if ($rawConfig) { Merge-OrderedMap (ConvertTo-OrderedMap $defaultConfig) $rawConfig } else { $defaultConfig }
$effectiveConfig['providerSelections'] = Merge-ProviderSelections -Defaults $defaultConfig['providerSelections'] -Configured $effectiveConfig['providerSelections']

Ensure-SetupDirectories @(
    $context.GalStateRoot,
    $context.GalConfigRoot,
    $context.GalStateDirectory,
    $context.GalPluginsRoot,
    $context.GalDataRoot,
    $context.GalCacheRoot,
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

if ($primaryProviders.Count -eq 0) {
    $primaryProviders = @(Get-ProviderNamesFromSelectedRuntimesByLane -SelectedRuntimes $selection.SelectedRuntimes -Lane 'primary')
}

if (
    $primaryProviders.Count -eq 0 -and
    -not [string]::IsNullOrWhiteSpace($selection.PrimaryRuntime) -and
    (Get-ProviderLaneForRuntime $selection.PrimaryRuntime) -eq 'primary'
) {
    $primaryProviders = @((Get-ProviderFromRuntime $selection.PrimaryRuntime))
}

if ($bridgeProviders.Count -eq 0) {
    $bridgeProviders = @(Get-ProviderNamesFromSelectedRuntimesByLane -SelectedRuntimes $selection.SelectedRuntimes -Lane 'bridge')
}

if ($migrationProviders.Count -eq 0) {
    $migrationProviders = @(Get-ProviderNamesFromSelectedRuntimesByLane -SelectedRuntimes $selection.SelectedRuntimes -Lane 'migration')
}

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

    if ($primaryProviders -contains 'claude') {
        Write-Host '  [OK] Source mode still projects Claude through the plugin-shaped surface.'
        & (Join-Path $PSScriptRoot 'Build-ProviderPlugins.ps1') -RepoRoot $RepoRoot -ConfigPath $resolverConfigPath -LockfilePath $resolverLockfilePath -Providers @('claude') -DryRun:$DryRun -Force:$Force
        Invoke-ClaudePluginLifecycle -RepoRoot $RepoRoot -Context $context
    }
}
else {
    Write-Host ("  [OK] Install mode projections root: {0}" -f $context.GalGeneratedRoot)
    Write-Host '  [OK] Install mode disables repo-root links and source-only local overrides.'
    if ($BootstrapInstall -and -not $configExists) {
        Write-Host '  [OK] First launch bootstrap path seeded install mode because no machine config existed yet.'
    }
    if ($primaryProviders.Count -gt 0) {
        & (Join-Path $PSScriptRoot 'Build-ProviderPlugins.ps1') -RepoRoot $RepoRoot -ConfigPath $resolverConfigPath -LockfilePath $resolverLockfilePath -Providers $primaryProviders -DryRun:$DryRun -Force:$Force
        if ($primaryProviders -contains 'copilot') {
            Invoke-CopilotPluginLifecycle -RepoRoot $RepoRoot -Context $context
        }
        if ($primaryProviders -contains 'codex') {
            Invoke-CodexPluginLifecycle -RepoRoot $RepoRoot -Context $context
        }
        if ($primaryProviders -contains 'claude') {
            Invoke-ClaudePluginLifecycle -RepoRoot $RepoRoot -Context $context
        }
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
