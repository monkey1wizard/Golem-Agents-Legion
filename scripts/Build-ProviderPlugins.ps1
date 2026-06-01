#Requires -Version 5.1

[CmdletBinding()]
param(
    [string]$RepoRoot,
    [string]$CatalogPath,
    [string]$ConfigPath = (Join-Path $env:USERPROFILE '.gal\config\config.json'),
    [string]$LockfilePath = (Join-Path $env:USERPROFILE '.gal\state\plugins.lock.json'),
    [string[]]$Providers = @('agy', 'copilot', 'codex', 'claude'),
    [switch]$DryRun,
    [switch]$Force,
    [switch]$PassThru
)

$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
    $RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
}
if ([string]::IsNullOrWhiteSpace($CatalogPath)) {
    $CatalogPath = Join-Path $RepoRoot 'plugins\catalog.json'
}

. (Join-Path (Join-Path $PSScriptRoot 'common') 'ProviderPlugin.ps1')
. (Join-Path (Join-Path $PSScriptRoot 'common') 'Common.ps1')

if (-not (Get-Variable -Scope Script -Name SetupContext -ErrorAction SilentlyContinue)) {
    $script:SetupContext = New-SetupContext -EntryScriptPath $MyInvocation.MyCommand.Path
}

if (-not (Get-Variable -Scope Script -Name SetupOptions -ErrorAction SilentlyContinue)) {
    $script:SetupOptions = [pscustomobject]@{
        Uninstall = $false
        Replace = $false
        DryRun = $DryRun.IsPresent
        Reconfigure = $false
    }
}

function Set-ProviderShortcutTarget {
    param(
        [Parameter(Mandatory)]
        [string]$Provider,
        [Parameter(Mandatory)]
        [string]$TargetPath
    )

    $shortcutPath = Get-GalActiveProviderTarget -Provider $Provider
    $shortcutParent = Split-Path $shortcutPath -Parent
    if (-not (Test-Path $shortcutParent)) {
        New-Item -ItemType Directory -Path $shortcutParent -Force | Out-Null
    }

    $linked = New-SafeSymlink -LinkPath $shortcutPath -TargetPath $TargetPath -Type 'Directory'
    if (-not $linked) {
        throw "Failed to claim GAL-managed shortcut for provider '$Provider': $shortcutPath"
    }
}

function Write-ProviderManagedStateFromBuildPlan {
    param(
        [pscustomobject]$Plan,
        [pscustomobject]$Context,
        [bool]$DryRunMode
    )

    $projectionRoot = if ($Plan.Provider -eq 'agy') { $Plan.InstallTarget } else { $null }
    $status = Resolve-ProviderManagedStateStatus -Mode $Plan.Mode -LifecycleStatus $Plan.LifecycleStatus -ProjectionRoot $projectionRoot
    $readSurface = Resolve-ProviderManagedReadSurface -Status $status
    $statePath = Get-ProviderManagedStatePath -Provider $Plan.Provider -Context $Context
    $state = [ordered]@{
        schemaVersion = 1
        provider = $Plan.Provider
        canonicalRoot = $Plan.CanonicalRoot
        packageOutputRoot = $Plan.PackageOutputRoot
        projectionRoot = $projectionRoot
        installTarget = $Plan.InstallTarget
        shortcutTarget = $Plan.ShortcutTarget
        generatedAt = (Get-Date -Format 'o')
        status = $status
        readSurface = $readSurface
        cli = [ordered]@{
            available = $false
            validateSupported = $false
            localArtifactInstallSupported = $false
            marketplaceInstallSupported = $false
            installScopeSupported = $false
            installHelpSummary = $null
        }
        validation = [ordered]@{
            command = $null
            strictPassed = $false
        }
        lifecycle = [ordered]@{
            mode = $Plan.Mode
            status = $Plan.LifecycleStatus
        }
        notes = @(
            'Base provider ledger written from the provider build plan.',
            'Later lifecycle-specific tasks may enrich this file with provider-native validation and install details.'
        )
    }

    if ($DryRunMode) {
        Write-Host ("[DRY RUN] Would write {0} managed state: {1}" -f $Plan.Provider, $statePath)
        return
    }

    Write-JsonOrderedMap $statePath $state
    Write-Host ("[OK] Wrote {0} managed state: {1}" -f $Plan.Provider, $statePath)
}

$resolverScript = Join-Path $PSScriptRoot 'Resolve-GalCatalog.ps1'
if (-not (Test-Path $resolverScript)) {
    throw "Catalog resolver not found: $resolverScript"
}

$effectiveLockfilePath = if ($DryRun) {
    Join-Path $env:TEMP ("gal-provider-build-lock-{0}.json" -f [System.Guid]::NewGuid().ToString('N'))
}
else {
    $LockfilePath
}

$resolution = & $resolverScript -CatalogPath $CatalogPath -ConfigPath $ConfigPath -LockfilePath $effectiveLockfilePath -PassThru
if (@($resolution.Errors).Count -gt 0) {
    throw "Catalog resolution failed: $($resolution.Errors -join '; ')"
}

$buildPlan = [System.Collections.Generic.List[object]]::new()
foreach ($provider in $Providers) {
    switch ($provider) {
        'agy' {
            $buildPlan.Add([pscustomobject]@{
                Provider = 'agy'
                Mode = 'managed-shortcut'
                Renderer = 'Build-CorePlugin.ps1'
                CanonicalRoot = Get-GalPluginRoot -PluginId 'gal'
                PackageOutputRoot = Get-GalPluginRoot -PluginId 'gal'
                InstallTarget = Get-AgyPluginInstallTarget
                ShortcutTarget = Get-GalActiveProviderTarget -Provider 'agy'
                LifecycleStatus = 'implemented'
            })
        }
        'copilot' {
            $buildPlan.Add([pscustomobject]@{
                Provider = 'copilot'
                Mode = 'native-install'
                Renderer = 'Build-CorePlugin.ps1'
                CanonicalRoot = Get-GalPluginRoot -PluginId 'gal'
                PackageOutputRoot = Get-GalPluginRoot -PluginId 'gal'
                InstallTarget = 'provider-managed via gh copilot plugin install <canonical-root>'
                ShortcutTarget = $null
                LifecycleStatus = 'artifact-rendered-install-deferred'
            })
        }
        'codex' {
            $buildPlan.Add([pscustomobject]@{
                Provider = 'codex'
                Mode = 'native-install'
                Renderer = 'Build-CorePlugin.ps1'
                CanonicalRoot = Get-GalPluginRoot -PluginId 'gal'
                PackageOutputRoot = Get-GalPluginRoot -PluginId 'gal'
                InstallTarget = 'provider-managed via codex plugin marketplace add + codex plugin add gal@gal-marketplace'
                ShortcutTarget = $null
                LifecycleStatus = 'artifact-rendered-install-deferred'
            })
        }
        'claude' {
            $buildPlan.Add([pscustomobject]@{
                Provider = 'claude'
                Mode = 'native-install'
                Renderer = 'Build-CorePlugin.ps1'
                CanonicalRoot = Get-GalPluginRoot -PluginId 'gal'
                PackageOutputRoot = Get-GalPluginRoot -PluginId 'gal'
                InstallTarget = 'provider-managed via claude plugin install --scope <scope>'
                ShortcutTarget = $null
                LifecycleStatus = 'artifact-rendered-install-deferred'
            })
        }
        default {
            throw "Unsupported provider: $provider"
        }
    }
}

$tempResolvedPluginsFile = Join-Path $env:TEMP ("gal-resolved-plugins-{0}.json" -f [System.Guid]::NewGuid().ToString('N'))
$resolution.ResolvedPlugins | ConvertTo-Json -Depth 20 | Set-Content -Path $tempResolvedPluginsFile -Encoding UTF8

try {
    if ($DryRun) {
        Write-Host '--- DRY RUN ---'
        Write-Host "Resolved plugins: $(($resolution.ResolvedPlugins | ForEach-Object { $_.pluginId }) -join ', ')"
        foreach ($plan in $buildPlan) {
            $shortcutTarget = if ([string]::IsNullOrWhiteSpace($plan.ShortcutTarget)) { 'none' } else { $plan.ShortcutTarget }
            Write-Host ("[{0}] mode={1} renderer={2} shortcut={3}" -f $plan.Provider, $plan.Mode, $plan.Renderer, $shortcutTarget)
        }
    }
    else {
        $canonicalPluginRendered = $false
        foreach ($plan in $buildPlan) {
            if ($plan.Renderer -ne 'Build-CorePlugin.ps1') {
                continue
            }

            if ($plan.Provider -eq 'agy') {
                # AGY is the only lane that currently projects managed shortcuts during the build step.
                & (Join-Path $PSScriptRoot 'Build-CorePlugin.ps1') -RepoRoot $RepoRoot -ResolvedPluginsFile $tempResolvedPluginsFile -Install -Force:$Force
                $canonicalPluginRendered = $true
                Set-ProviderShortcutTarget -Provider 'agy' -TargetPath $plan.InstallTarget
                continue
            }

            if (-not $canonicalPluginRendered) {
                # Render the shared canonical root once for any selected native-install lane.
                & (Join-Path $PSScriptRoot 'Build-CorePlugin.ps1') -RepoRoot $RepoRoot -ResolvedPluginsFile $tempResolvedPluginsFile -Force:$Force
                $canonicalPluginRendered = $true
            }
        }

        foreach ($plan in $buildPlan) {
            Write-ProviderManagedStateFromBuildPlan -Plan $plan -Context $script:SetupContext -DryRunMode:$false
        }
    }

    if ($PassThru) {
        return [pscustomobject]@{
            ResolvedPlugins = $resolution.ResolvedPlugins
            BuildPlan = @($buildPlan)
            ResolvedPluginsFile = $tempResolvedPluginsFile
        }
    }
}
finally {
    if ((-not $PassThru) -and (Test-Path $tempResolvedPluginsFile)) {
        Remove-Item -LiteralPath $tempResolvedPluginsFile -Force
    }
    if ($DryRun -and (Test-Path $effectiveLockfilePath)) {
        Remove-Item -LiteralPath $effectiveLockfilePath -Force
    }
}
