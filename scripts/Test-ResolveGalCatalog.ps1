#Requires -Version 5.1

<#
.SYNOPSIS
    T-005 automated tests for catalog resolver.
#>

$ErrorActionPreference = 'Stop'

$results = [System.Collections.Generic.List[string]]::new()
$passCount = 0
$failCount = 0

function Assert-Equal {
    param($Expected, $Actual, $Label)
    if ($Expected -eq $Actual) {
        $script:passCount++
        $script:results.Add("PASS: $Label")
    } else {
        $script:failCount++
        $script:results.Add("FAIL: $Label (expected='$Expected', actual='$Actual')")
    }
}

function Assert-True {
    param($Condition, $Label)
    if ($Condition) {
        $script:passCount++
        $script:results.Add("PASS: $Label")
    } else {
        $script:failCount++
        $script:results.Add("FAIL: $Label")
    }
}

# --- Setup ---
$repoRoot = Split-Path $PSScriptRoot -Parent
$catalogPath = Join-Path (Join-Path $repoRoot 'plugins') 'catalog.json'
$defaultConfig = @{ defaultProfile = 'default' } | ConvertTo-Json
$dartConfig = @{ defaultProfile = 'dart' } | ConvertTo-Json
$fullConfig = @{ defaultProfile = 'full' } | ConvertTo-Json
$explicitConfig = @{ enabledPlugins = @('gal-core','rust-skills') } | ConvertTo-Json

$defaultConfigPath = Join-Path $env:TEMP 'gal-test-default.json'
$dartConfigPath = Join-Path $env:TEMP 'gal-test-dart.json'
$fullConfigPath = Join-Path $env:TEMP 'gal-test-full.json'
$explicitConfigPath = Join-Path $env:TEMP 'gal-test-explicit.json'

$defaultConfig | Set-Content $defaultConfigPath -Encoding UTF8
$dartConfig | Set-Content $dartConfigPath -Encoding UTF8
$fullConfig | Set-Content $fullConfigPath -Encoding UTF8
$explicitConfig | Set-Content $explicitConfigPath -Encoding UTF8

$lockfileBase = Join-Path $env:TEMP 'gal-test-lock'

# --- TP-004: Default profile resolves only gal-core ---
$out = & (Join-Path $PSScriptRoot 'Resolve-GalCatalog.ps1') -CatalogPath $catalogPath -ConfigPath $defaultConfigPath -LockfilePath "$lockfileBase-default.json" -PassThru
$ids = $out.ResolvedPlugins | ForEach-Object { $_.pluginId }
Assert-Equal -Expected 'gal-core' -Actual ($ids -join ',') -Label 'TP-004: default profile resolves only gal-core'

# --- TP-004: Named profile resolves gal-core + companion ---
$out = & (Join-Path $PSScriptRoot 'Resolve-GalCatalog.ps1') -CatalogPath $catalogPath -ConfigPath $dartConfigPath -LockfilePath "$lockfileBase-dart.json" -PassThru
$ids = $out.ResolvedPlugins | ForEach-Object { $_.pluginId }
Assert-Equal -Expected 'gal-core,dart-skills' -Actual ($ids -join ',') -Label 'TP-004: dart profile resolves gal-core + dart-skills'

# --- TP-004: Full profile resolves all plugins ---
$out = & (Join-Path $PSScriptRoot 'Resolve-GalCatalog.ps1') -CatalogPath $catalogPath -ConfigPath $fullConfigPath -LockfilePath "$lockfileBase-full.json" -PassThru
$ids = $out.ResolvedPlugins | ForEach-Object { $_.pluginId }
Assert-True -Condition ($ids.Count -eq 9) -Label 'TP-004: full profile resolves 9 plugins'

# --- TP-004: Explicit plugin selection ---
$out = & (Join-Path $PSScriptRoot 'Resolve-GalCatalog.ps1') -CatalogPath $catalogPath -ConfigPath $explicitConfigPath -LockfilePath "$lockfileBase-explicit.json" -PassThru
$ids = $out.ResolvedPlugins | ForEach-Object { $_.pluginId }
Assert-Equal -Expected 'gal-core,rust-skills' -Actual ($ids -join ',') -Label 'TP-004: explicit enabledPlugins resolves gal-core + rust-skills'

# --- TP-005: Deterministic resolution ---
$out1 = & (Join-Path $PSScriptRoot 'Resolve-GalCatalog.ps1') -CatalogPath $catalogPath -ConfigPath $defaultConfigPath -LockfilePath "$lockfileBase-det1.json" -PassThru
$out2 = & (Join-Path $PSScriptRoot 'Resolve-GalCatalog.ps1') -CatalogPath $catalogPath -ConfigPath $defaultConfigPath -LockfilePath "$lockfileBase-det2.json" -PassThru
$ids1 = $out1.ResolvedPlugins | ForEach-Object { $_.pluginId }
$ids2 = $out2.ResolvedPlugins | ForEach-Object { $_.pluginId }
Assert-True -Condition (($ids1 -join ',') -eq ($ids2 -join ',')) -Label 'TP-005: same input produces same plugin set'
Assert-True -Condition ($out1.Lockfile.catalogHash -eq $out2.Lockfile.catalogHash) -Label 'TP-005: same input produces same catalog hash'

# --- TP-006: Catalog validation rejects missing metadata ---
$badCatalog = Get-Content $catalogPath -Raw | ConvertFrom-Json
$badPlugin = @{
    pluginId = 'bad-plugin'
    displayName = 'Bad Plugin'
    supportTier = 'curated-upstream'
    sourceType = 'curated-upstream'
    upstream = @{ repo = 'https://github.com/example/bad' }
    license = ''
    checksumPolicy = ''
    componentMap = @{ skills = $true }
    supportedProviders = @('claude')
    installStrategy = 'git-clone'
    defaultProfiles = @('bad')
    allowAutoUpdate = $false
    localOverridePolicy = 'source-mode-only'
}
$badCatalog.plugins += $badPlugin
$badCatalogPath = Join-Path $env:TEMP 'gal-test-bad-catalog.json'
$badCatalog | ConvertTo-Json -Depth 20 | Set-Content $badCatalogPath -Encoding UTF8

$out = & (Join-Path $PSScriptRoot 'Resolve-GalCatalog.ps1') -CatalogPath $badCatalogPath -ConfigPath $defaultConfigPath -LockfilePath "$lockfileBase-bad.json" -PassThru -ErrorAction Continue
Assert-True -Condition ($out.Errors.Count -gt 0) -Label 'TP-006: missing license/checksum/ref causes validation errors'
Assert-True -Condition ($out.Errors -match 'license') -Label 'TP-006: error mentions missing license'
Assert-True -Condition ($out.Errors -match 'checksumPolicy') -Label 'TP-006: error mentions missing checksumPolicy'

# --- Drift detection ---
# First run creates lockfile, second run with same catalog should not detect drift
$lockfileDrift = Join-Path $env:TEMP 'gal-test-drift.json'
$out1 = & (Join-Path $PSScriptRoot 'Resolve-GalCatalog.ps1') -CatalogPath $catalogPath -ConfigPath $defaultConfigPath -LockfilePath $lockfileDrift -PassThru
$out2 = & (Join-Path $PSScriptRoot 'Resolve-GalCatalog.ps1') -CatalogPath $catalogPath -ConfigPath $defaultConfigPath -LockfilePath $lockfileDrift -PassThru
Assert-True -Condition (-not $out1.DriftDetected) -Label 'TP-005: first run does not detect drift'
Assert-True -Condition (-not $out2.DriftDetected) -Label 'TP-005: second identical run does not detect drift'

# --- Report ---
Write-Host ""
Write-Host "=== T-005 Test Results ==="
Write-Host "Passed: $passCount"
Write-Host "Failed: $failCount"
Write-Host ""
foreach ($r in $results) { Write-Host $r }

if ($failCount -gt 0) { exit 1 }
