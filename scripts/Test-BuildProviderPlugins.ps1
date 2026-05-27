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
$releaseMatrixPath = Join-Path $repoRoot 'docs\release-matrix.md'

(@{ defaultProfile = 'full' } | ConvertTo-Json) | Set-Content -Path $configPath -Encoding UTF8

$plan = & (Join-Path $PSScriptRoot 'Build-ProviderPlugins.ps1') -RepoRoot $repoRoot -ConfigPath $configPath -LockfilePath $lockfilePath -DryRun -PassThru

$agyPlan = $plan.BuildPlan | Where-Object Provider -eq 'agy' | Select-Object -First 1
$copilotPlan = $plan.BuildPlan | Where-Object Provider -eq 'copilot' | Select-Object -First 1
$codexPlan = $plan.BuildPlan | Where-Object Provider -eq 'codex' | Select-Object -First 1
$claudePlan = $plan.BuildPlan | Where-Object Provider -eq 'claude' | Select-Object -First 1

. (Join-Path $PSScriptRoot 'common\ProviderPlugin.ps1')
$canonicalSchema = Get-GalCoreCanonicalPackageSchema
$canonicalPackage = New-ProviderPluginPackage -RepoRoot $repoRoot -ResolvedPlugins $null
$canonicalValidation = Test-ProviderPluginPackage -Package $canonicalPackage
$releaseMatrix = Get-Content -LiteralPath $releaseMatrixPath -Raw

Assert-True -Condition ($agyPlan.Mode -eq 'managed-shortcut') -Label 'TP-010: AGY build plan uses managed shortcut mode'
Assert-True -Condition ($agyPlan.ShortcutTarget -like '*\.gal\active\agy') -Label 'TP-010: AGY shortcut target points to ~/.gal/active/agy'
Assert-True -Condition ($copilotPlan.Mode -eq 'native-install') -Label 'TP-010: Copilot stays on native-install lane'
Assert-True -Condition ($null -eq $copilotPlan.ShortcutTarget) -Label 'TP-010: Copilot does not get a managed shortcut target'
Assert-True -Condition ($copilotPlan.Renderer -eq 'not-yet-implemented') -Label 'TP-016: Copilot direct install is not yet claimed before renderer verification'
Assert-True -Condition ($codexPlan.Mode -eq 'native-install') -Label 'TP-006: Codex stays on native-install lane'
Assert-True -Condition ($null -eq $codexPlan.ShortcutTarget) -Label 'TP-006: Codex does not get a managed shortcut target'
Assert-True -Condition ($codexPlan.Renderer -eq 'not-yet-implemented') -Label 'TP-016: Codex direct install is not yet claimed before renderer verification'
Assert-True -Condition ($claudePlan.Mode -eq 'native-install') -Label 'TP-006: Claude remains the baseline native-install lane'
Assert-True -Condition ($claudePlan.Renderer -eq 'not-yet-implemented') -Label 'TP-016: Claude direct install is not yet claimed before renderer verification'
Assert-True -Condition ($canonicalSchema.canonicalProvider -eq 'claude') -Label 'TP-006: Claude is the canonical package schema baseline'
Assert-True -Condition ($canonicalSchema.compatibleProviders -contains 'copilot') -Label 'TP-006: Copilot stays in the canonical compatibility set'
Assert-True -Condition ($canonicalSchema.compatibleProviders -contains 'codex') -Label 'TP-006: Codex stays in the canonical compatibility set'
Assert-True -Condition ($canonicalSchema.nativeInstallProviders -contains 'claude') -Label 'TP-006: Claude stays in the native-install provider set'
Assert-True -Condition ($canonicalSchema.nativeInstallProviders -contains 'copilot') -Label 'TP-006: Copilot stays in the native-install provider set'
Assert-True -Condition ($canonicalSchema.nativeInstallProviders -contains 'codex') -Label 'TP-006: Codex stays in the native-install provider set'
Assert-True -Condition $canonicalValidation.Valid -Label 'TP-009: Claude-compatible canonical package passes shared substrate validation'
Assert-True -Condition ($canonicalPackage.providerCapabilities.claude.skills -and $canonicalPackage.providerCapabilities.claude.commandSkills -and $canonicalPackage.providerCapabilities.claude.agents -and $canonicalPackage.providerCapabilities.claude.instructions -and $canonicalPackage.providerCapabilities.claude.mcp) -Label 'TP-009: Claude baseline package exposes the documented Claude-readable components'
Assert-True -Condition ($canonicalPackage.providerCapabilities.copilot.skills -and $canonicalPackage.providerCapabilities.copilot.commandSkills -and $canonicalPackage.providerCapabilities.copilot.agents -and $canonicalPackage.providerCapabilities.copilot.instructions -and $canonicalPackage.providerCapabilities.copilot.mcp) -Label 'TP-006: Copilot marketplace package exposes the documented Copilot-readable components'
Assert-True -Condition ($canonicalPackage.providerCapabilities.codex.skills -and $canonicalPackage.providerCapabilities.codex.commandSkills -and (-not $canonicalPackage.providerCapabilities.codex.agents) -and $canonicalPackage.providerCapabilities.codex.instructions -and $canonicalPackage.providerCapabilities.codex.mcp) -Label 'TP-006: Codex marketplace package exposes the documented Codex-readable components'
Assert-True -Condition (($canonicalPackage.skippedComponents -contains 'hooks') -and ($canonicalPackage.skippedComponents -contains 'runtimeScripts')) -Label 'TP-016: Claude baseline package does not claim unsupported hook or runtime-script components'
Assert-True -Condition ($releaseMatrix.Contains('do not create a Codex-only version stream.') -and $releaseMatrix.Contains('the Codex wrapper must describe the same canonical package lineage')) -Label 'TP-006: Codex submission artifact policy stays aligned to the canonical package lineage'
Assert-True -Condition ($releaseMatrix.Contains('do not create a Copilot-only version stream.') -and $releaseMatrix.Contains('the Copilot wrapper must describe the same canonical package lineage')) -Label 'TP-006: Copilot submission artifact policy stays aligned to the canonical package lineage'
Assert-True -Condition ($releaseMatrix.Contains('if Codex review or publication lags the canonical release by more than 5 business days, the entry must explicitly direct users to GitHub Releases.') -and $releaseMatrix.Contains('if Copilot review or publication lags the canonical release by more than 5 business days, the entry must explicitly direct users to GitHub Releases.')) -Label 'TP-016: Codex and Copilot fallback-link policy stays explicit when downstream publication lags'

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
Write-Host '=== Provider Build Test Results ==='
Write-Host "Passed: $passCount"
Write-Host "Failed: $failCount"
Write-Host ''
foreach ($result in $results) {
    Write-Host $result
}

if ($failCount -gt 0) {
    exit 1
}