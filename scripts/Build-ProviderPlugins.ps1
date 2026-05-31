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
                LifecycleStatus = 'implemented'
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
                LifecycleStatus = 'implemented'
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
            if ($plan.Provider -eq 'agy') {
                # Core renderer builds superset canonical root AND installs all 3 AGY surfaces
                & (Join-Path $PSScriptRoot 'Build-CorePlugin.ps1') -RepoRoot $RepoRoot -ResolvedPluginsFile $tempResolvedPluginsFile -Install -Force:$Force
                $canonicalPluginRendered = $true
                Set-ProviderShortcutTarget -Provider 'agy' -TargetPath $plan.InstallTarget
                continue
            }

            if ($plan.Provider -eq 'claude') {
                if (-not $canonicalPluginRendered) {
                    # Core renderer without -Install (Claude uses marketplace; no surface projection needed)
                    & (Join-Path $PSScriptRoot 'Build-CorePlugin.ps1') -RepoRoot $RepoRoot -ResolvedPluginsFile $tempResolvedPluginsFile -Force:$Force
                    $canonicalPluginRendered = $true
                }
            }
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
