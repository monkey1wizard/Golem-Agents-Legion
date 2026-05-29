#Requires -Version 5.1

<#!
.SYNOPSIS
    T-006 automated tests for the gal-core canonical package builder.
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
    }
    else {
        $script:failCount++
        $script:results.Add("FAIL: $Label (expected='$Expected', actual='$Actual')")
    }
}

function Assert-True {
    param($Condition, $Label)
    if ($Condition) {
        $script:passCount++
        $script:results.Add("PASS: $Label")
    }
    else {
        $script:failCount++
        $script:results.Add("FAIL: $Label")
    }
}

$repoRoot = Split-Path $PSScriptRoot -Parent
. (Join-Path (Join-Path $PSScriptRoot 'common') 'ProviderPlugin.ps1')

$catalogPath = Join-Path $repoRoot 'plugins\catalog.json'
$resolveScript = Join-Path $PSScriptRoot 'Resolve-GalCatalog.ps1'
$configDefaultPath = Join-Path $env:TEMP 'gal-test-provider-package-default.json'
$configFullPath = Join-Path $env:TEMP 'gal-test-provider-package-full.json'
$lockDefaultPath = Join-Path $env:TEMP 'gal-test-provider-package-default.lock.json'
$lockFullPath = Join-Path $env:TEMP 'gal-test-provider-package-full.lock.json'

(@{ defaultProfile = 'default' } | ConvertTo-Json) | Set-Content $configDefaultPath -Encoding UTF8
(@{ defaultProfile = 'full' } | ConvertTo-Json) | Set-Content $configFullPath -Encoding UTF8

$defaultResolution = & $resolveScript -CatalogPath $catalogPath -ConfigPath $configDefaultPath -LockfilePath $lockDefaultPath -PassThru
$fullResolution = & $resolveScript -CatalogPath $catalogPath -ConfigPath $configFullPath -LockfilePath $lockFullPath -PassThru

$defaultPackage = New-ProviderPluginPackage -RepoRoot $repoRoot -ResolvedPlugins $defaultResolution.ResolvedPlugins
$fullPackage = New-ProviderPluginPackage -RepoRoot $repoRoot -ResolvedPlugins $fullResolution.ResolvedPlugins

$defaultSkillNames = @($defaultPackage.skills | ForEach-Object { $_.name })
$fullDeferredPlugins = @($fullPackage.deferredCompanionPlugins | ForEach-Object { $_.pluginId })

Assert-Equal -Expected 'gal-core' -Actual $defaultPackage.packageSchema.packageId -Label 'TP-008: canonical package schema identifies gal-core'
Assert-Equal -Expected 'claude' -Actual $defaultPackage.packageSchema.canonicalProvider -Label 'TP-008: canonical provider is Claude'
Assert-True -Condition ($defaultSkillNames -contains 'godot-scripting') -Label 'TP-008: gal-core keeps GAL-owned skills'
Assert-True -Condition (-not ($defaultSkillNames | Where-Object { $_ -like 'dart-*' })) -Label 'TP-008: dart companion skills are excluded from gal-core package'
Assert-True -Condition (-not ($defaultSkillNames | Where-Object { $_ -like 'flutter-*' })) -Label 'TP-008: flutter companion skills are excluded from gal-core package'
Assert-True -Condition ($fullDeferredPlugins -contains 'dart-skills') -Label 'TP-008: resolved companion plugins are recorded as deferred metadata'
Assert-True -Condition ($fullDeferredPlugins -contains 'flutter-skills') -Label 'TP-008: multiple companion plugins remain outside gal-core payload'

Write-Host ''
Write-Host '=== T-006 Test Results ==='
Write-Host "Passed: $passCount"
Write-Host "Failed: $failCount"
Write-Host ''
foreach ($result in $results) {
    Write-Host $result
}

if ($failCount -gt 0) {
    exit 1
}
