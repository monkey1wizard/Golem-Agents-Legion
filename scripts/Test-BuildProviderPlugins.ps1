#Requires -Version 5.1

$ErrorActionPreference = 'Stop'

$results = [System.Collections.Generic.List[string]]::new()
$passCount = 0
$failCount = 0

function Assert-True {
    param(
        [bool]$Condition,
        [string]$Label
    )

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
$configPath = Join-Path $env:TEMP 'gal-test-provider-build-full.json'
$lockfilePath = Join-Path $env:TEMP 'gal-test-provider-build-full.lock.json'

(@{ defaultProfile = 'full' } | ConvertTo-Json) | Set-Content -Path $configPath -Encoding UTF8

$plan = & (Join-Path $PSScriptRoot 'Build-ProviderPlugins.ps1') -RepoRoot $repoRoot -ConfigPath $configPath -LockfilePath $lockfilePath -DryRun -PassThru

$agyPlan = $plan.BuildPlan | Where-Object Provider -eq 'agy' | Select-Object -First 1
$copilotPlan = $plan.BuildPlan | Where-Object Provider -eq 'copilot' | Select-Object -First 1

Assert-True -Condition ($agyPlan.Mode -eq 'managed-shortcut') -Label 'TP-010: AGY build plan uses managed shortcut mode'
Assert-True -Condition ($agyPlan.ShortcutTarget -like '*\.gal\active\agy') -Label 'TP-010: AGY shortcut target points to ~/.gal/active/agy'
Assert-True -Condition ($copilotPlan.Mode -eq 'native-install') -Label 'TP-010: Copilot stays on native-install lane'
Assert-True -Condition ($null -eq $copilotPlan.ShortcutTarget) -Label 'TP-010: Copilot does not get a managed shortcut target'

$resolvedJsonPath = $plan.ResolvedPluginsFile
try {
    & (Join-Path $PSScriptRoot 'Build-AgyPlugin.ps1') -RepoRoot $repoRoot -ResolvedPluginsFile $resolvedJsonPath -Force | Out-Null
    $pluginJson = Get-Content -LiteralPath (Join-Path $repoRoot 'dist/provider-plugins/agy/gal/plugin.json') -Raw | ConvertFrom-Json

    Assert-True -Condition ($pluginJson.canonicalPackage.packageId -eq 'gal-core') -Label 'TP-009: AGY manifest preserves canonical package identity'
    Assert-True -Condition ($pluginJson.deferredCompanionPlugins.pluginId -contains 'dart-skills') -Label 'TP-009: AGY manifest preserves deferred companion identity'
}
finally {
    if (Test-Path $resolvedJsonPath) {
        Remove-Item -LiteralPath $resolvedJsonPath -Force
    }
}

Write-Host ''
Write-Host '=== T-007 Test Results ==='
Write-Host "Passed: $passCount"
Write-Host "Failed: $failCount"
Write-Host ''
foreach ($result in $results) {
    Write-Host $result
}

if ($failCount -gt 0) {
    exit 1
}