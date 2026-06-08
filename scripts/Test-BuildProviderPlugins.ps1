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

function Test-LinkTargetsCanonical {
    param(
        [string]$Path,
        [string]$ExpectedTarget
    )

    if (-not (Test-Path $Path)) {
        return $false
    }

    $item = Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
    if ($null -eq $item) {
        return $false
    }

    $isReparsePoint = ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0
    if (-not $isReparsePoint) {
        return $false
    }

    foreach ($target in @($item.Target)) {
        if ([string]::IsNullOrWhiteSpace([string]$target)) {
            continue
        }

        if (([string]$target).Replace('/', '\') -eq $ExpectedTarget.Replace('/', '\')) {
            return $true
        }
    }

    return $false
}

function Copy-TestSourceRoot {
    param(
        [Parameter(Mandatory)][string]$SourceRoot,
        [Parameter(Mandatory)][string]$DestinationRoot
    )

    $entries = @(
        'plugins',
        '.dev\project.md'
    )

    New-Item -ItemType Directory -Path $DestinationRoot -Force | Out-Null
    foreach ($entry in $entries) {
        $sourcePath = Join-Path $SourceRoot $entry
        $destinationPath = Join-Path $DestinationRoot $entry
        $destinationParent = Split-Path -Parent $destinationPath
        if (-not [string]::IsNullOrWhiteSpace($destinationParent)) {
            New-Item -ItemType Directory -Path $destinationParent -Force | Out-Null
        }

        Copy-Item -LiteralPath $sourcePath -Destination $destinationPath -Recurse -Force
    }
}

    $repoRoot = Split-Path $PSScriptRoot -Parent
    $releaseMatrixPath = Join-Path $repoRoot 'docs\devguide.md'

$testHome = Join-Path $env:TEMP ("gal-test-provider-build-{0}" -f [System.Guid]::NewGuid().ToString('N'))
$originalUserProfile = $env:USERPROFILE
$originalHome = $env:HOME
$agyIdeTarget = Join-Path $testHome '.gemini\antigravity-ide\plugins\gal'
$agyGuiConfigTarget = Join-Path $testHome '.gemini\config\plugins\gal'
$mutableRepoRoot = Join-Path $testHome 'repo-copy'

New-Item -ItemType Directory -Path $testHome | Out-Null
Copy-TestSourceRoot -SourceRoot $repoRoot -DestinationRoot $mutableRepoRoot

try {
    $env:USERPROFILE = $testHome
    $env:HOME = $testHome

    $configPath = Join-Path $testHome '.gal\config\config.json'
    $lockfilePath = Join-Path $testHome '.gal\state\plugins.lock.json'
    New-Item -ItemType Directory -Path (Split-Path $configPath -Parent) -Force | Out-Null
    New-Item -ItemType Directory -Path (Split-Path $lockfilePath -Parent) -Force | Out-Null

    (@{ defaultProfile = 'full' } | ConvertTo-Json) | Set-Content -Path $configPath -Encoding UTF8

    $plan = & (Join-Path $PSScriptRoot 'Build-ProviderPlugins.ps1') -RepoRoot $repoRoot -ConfigPath $configPath -LockfilePath $lockfilePath -DryRun -PassThru

    $agyPlan = $plan.BuildPlan | Where-Object Provider -eq 'agy' | Select-Object -First 1
    $copilotPlan = $plan.BuildPlan | Where-Object Provider -eq 'copilot' | Select-Object -First 1
    $codexPlan = $plan.BuildPlan | Where-Object Provider -eq 'codex' | Select-Object -First 1
    $claudePlan = $plan.BuildPlan | Where-Object Provider -eq 'claude' | Select-Object -First 1

    $canonicalPluginRoot = Join-Path $testHome '.gal\plugins\gal'
    $claudeArtifactRoot = Join-Path $testHome '.gal\plugins\gal'
    $claudeManifestPath = Join-Path $claudeArtifactRoot '.claude-plugin/plugin.json'
    $claudeMcpPath = Join-Path $claudeArtifactRoot '.mcp.json'
    $claudeSkillPath = Join-Path $claudeArtifactRoot 'skills/defuddle/SKILL.md'
    $claudeCommandPath = Join-Path $claudeArtifactRoot 'commands/gal.md'
    $claudeAgentPath = Join-Path $claudeArtifactRoot 'agents/golem-reviewer.md'

    . (Join-Path $PSScriptRoot 'common\ProviderPlugin.ps1')
    $canonicalSchema = Get-GalCoreCanonicalPackageSchema
    $canonicalPackage = New-ProviderPluginPackage -RepoRoot $repoRoot -ResolvedPlugins $null
    $canonicalValidation = Test-ProviderPluginPackage -Package $canonicalPackage
    $releaseMatrix = Get-Content -LiteralPath $releaseMatrixPath -Raw

    Assert-True -Condition ((Resolve-ProviderManagedReadSurface -Status 'unsupported-lane') -eq $null) -Label 'TP-016: unsupported-lane does not masquerade as a read surface'

Assert-True -Condition ($agyPlan.Mode -eq 'managed-shortcut') -Label 'TP-002: AGY build plan uses managed shortcut mode'
Assert-True -Condition ($agyPlan.ShortcutTarget -like '*\.gal\active\agy') -Label 'TP-002: AGY shortcut target points to ~/.gal/active/agy'
Assert-True -Condition ($copilotPlan.Mode -eq 'native-install') -Label 'TP-002: Copilot stays on native-install lane'
Assert-True -Condition ($null -eq $copilotPlan.ShortcutTarget) -Label 'TP-002: Copilot does not get a managed shortcut target'
Assert-True -Condition ($copilotPlan.Renderer -eq 'Build-CorePlugin.ps1') -Label 'TP-005: Copilot build plan uses the core renderer'
Assert-True -Condition ($copilotPlan.LifecycleStatus -eq 'artifact-rendered-install-deferred') -Label 'TP-005: Copilot lifecycle honestly reports artifact-rendered/install-deferred after T-002'
Assert-True -Condition ($copilotPlan.CanonicalRoot -eq $canonicalPluginRoot) -Label 'TP-005: Copilot build plan canonical root is the superset plugin root'
Assert-True -Condition ($copilotPlan.PackageOutputRoot -eq $canonicalPluginRoot) -Label 'TP-005: Copilot build plan package output root is the canonical root'
Assert-True -Condition ($codexPlan.Mode -eq 'native-install') -Label 'TP-002: Codex stays on native-install lane'
Assert-True -Condition ($null -eq $codexPlan.ShortcutTarget) -Label 'TP-002: Codex does not get a managed shortcut target'
Assert-True -Condition ($codexPlan.Renderer -eq 'Build-CorePlugin.ps1') -Label 'TP-006: Codex build plan uses the core renderer'
Assert-True -Condition ($codexPlan.LifecycleStatus -eq 'artifact-rendered-install-deferred') -Label 'TP-006: Codex lifecycle honestly reports artifact-rendered/install-deferred before its native lane is wired'
Assert-True -Condition ($codexPlan.CanonicalRoot -eq $canonicalPluginRoot) -Label 'TP-006: Codex build plan canonical root is the superset plugin root'
Assert-True -Condition ($codexPlan.PackageOutputRoot -eq $canonicalPluginRoot) -Label 'TP-006: Codex build plan package output root is the canonical root'
Assert-True -Condition ($claudePlan.Mode -eq 'native-install') -Label 'TP-002: Claude remains the baseline native-install lane'
Assert-True -Condition ($claudePlan.Renderer -eq 'Build-CorePlugin.ps1') -Label 'TP-002: Claude build plan uses the core renderer'
Assert-True -Condition ($agyPlan.Renderer -eq 'Build-CorePlugin.ps1') -Label 'TP-002: AGY build plan uses the core renderer'
Assert-True -Condition ($agyPlan.CanonicalRoot -eq $canonicalPluginRoot) -Label 'TP-002: AGY build plan canonical root is the superset plugin root'
Assert-True -Condition ($agyPlan.PackageOutputRoot -eq $canonicalPluginRoot) -Label 'TP-002: AGY build plan package output root is the canonical root (link-first; no separate dist)'
Assert-True -Condition ($claudePlan.CanonicalRoot -eq $claudeArtifactRoot) -Label 'TP-002: Claude build plan canonical root is the superset plugin root'
Assert-True -Condition ($claudePlan.PackageOutputRoot -eq $canonicalPluginRoot) -Label 'TP-002: Claude build plan package output root is the canonical root (link-first; no separate dist)'
Assert-True -Condition ($claudePlan.InstallTarget -eq 'provider-managed via claude plugin install --scope <scope>') -Label 'TP-004: Claude build plan documents provider-managed install targeting'
Assert-True -Condition ($claudePlan.LifecycleStatus -eq 'artifact-rendered-install-deferred') -Label 'TP-004: Claude build plan distinguishes rendered artifact from direct-install verification'
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
        & (Join-Path $PSScriptRoot 'Build-CorePlugin.ps1') -RepoRoot $mutableRepoRoot -ResolvedPluginsFile $resolvedJsonPath -Force | Out-Null
        & (Join-Path $PSScriptRoot 'Build-ProviderPlugins.ps1') -RepoRoot $mutableRepoRoot -ConfigPath $configPath -LockfilePath $lockfilePath -Providers @('agy', 'copilot', 'codex', 'claude') -Force | Out-Null

        $agyLedgerPath = Join-Path $testHome '.gal\dist\providers\agy\managed.json'
        $copilotLedgerPath = Join-Path $testHome '.gal\dist\providers\copilot\managed.json'
        $codexLedgerPath = Join-Path $testHome '.gal\dist\providers\codex\managed.json'
        $claudeLedgerPath = Join-Path $testHome '.gal\dist\providers\claude\managed.json'

        $agyLedger = Get-Content -LiteralPath $agyLedgerPath -Raw | ConvertFrom-Json
        $copilotLedger = Get-Content -LiteralPath $copilotLedgerPath -Raw | ConvertFrom-Json
        $codexLedger = Get-Content -LiteralPath $codexLedgerPath -Raw | ConvertFrom-Json
        $claudeLedger = Get-Content -LiteralPath $claudeLedgerPath -Raw | ConvertFrom-Json

        Assert-True -Condition (Test-Path $agyLedgerPath) -Label 'TP-016: AGY build step writes managed.json ledger'
        Assert-True -Condition (Test-Path $copilotLedgerPath) -Label 'TP-016: Copilot build step writes managed.json ledger'
        Assert-True -Condition (Test-Path $codexLedgerPath) -Label 'TP-016: Codex build step writes managed.json ledger'
        Assert-True -Condition (Test-Path $claudeLedgerPath) -Label 'TP-016: Claude build step writes managed.json ledger'
        Assert-True -Condition ($agyLedger.status -eq 'linked-projection' -and $agyLedger.readSurface -eq 'linked-projection') -Label 'TP-016: AGY ledger records linked-projection read surface'
        Assert-True -Condition ($copilotLedger.status -eq 'unprojected-artifact' -and $copilotLedger.readSurface -eq 'unprojected-artifact') -Label 'TP-016: Copilot ledger records unprojected-artifact before native lifecycle wiring'
        Assert-True -Condition ($codexLedger.status -eq 'unprojected-artifact' -and $codexLedger.readSurface -eq 'unprojected-artifact') -Label 'TP-016: Codex ledger records unprojected-artifact before native lifecycle wiring'
        Assert-True -Condition ($claudeLedger.status -eq 'unprojected-artifact' -and $claudeLedger.readSurface -eq 'unprojected-artifact') -Label 'TP-016: Claude build ledger records unprojected-artifact before Claude lifecycle enrichment'
        Assert-True -Condition ($agyLedger.PSObject.Properties.Name -contains 'cli' -and $agyLedger.PSObject.Properties.Name -contains 'validation') -Label 'TP-016: build-ledger shared shape includes cli and validation fields'

        # AGY: root plugin.json in the canonical root
        $agyRootPluginJsonPath = Join-Path $canonicalPluginRoot 'plugin.json'
        $agyMcpConfigPath = Join-Path $canonicalPluginRoot 'mcp_config.json'
        $agyRulesPath = Join-Path $canonicalPluginRoot 'rules\gal.md'
        $pluginJson = Get-Content -LiteralPath $agyRootPluginJsonPath -Raw | ConvertFrom-Json

        $codexPluginManifestPath = Join-Path $canonicalPluginRoot '.codex-plugin\plugin.json'
        Assert-True -Condition (Test-Path $codexPluginManifestPath) -Label 'TP-007: Superset canonical root contains Codex .codex-plugin/plugin.json'
        $codexManifest = Get-Content -LiteralPath $codexPluginManifestPath -Raw | ConvertFrom-Json
        Assert-True -Condition ($codexManifest.name -eq 'gal') -Label 'TP-006: Codex manifest name is gal'
        Assert-True -Condition ($codexManifest.skills -eq './skills/') -Label 'TP-006: Codex manifest references skills/ path'
        Assert-True -Condition ($null -eq $codexManifest.agents) -Label 'TP-006: Codex manifest does not reference agents/ (providerCapabilities.codex.agents=false)'

        $copilotManifestPath = Join-Path $canonicalPluginRoot 'copilot-manifest.json'
        Assert-True -Condition (Test-Path $copilotManifestPath) -Label 'TP-007: Superset canonical root contains Copilot copilot-manifest.json'
        $copilotManifest = Get-Content -LiteralPath $copilotManifestPath -Raw | ConvertFrom-Json
        Assert-True -Condition ($copilotManifest.components.agents -eq 'agents/') -Label 'TP-005: Copilot manifest explicitly exposes agents/ path (fixes BUG-02)'
        Assert-True -Condition ($copilotManifest.components.skills -eq 'skills/') -Label 'TP-005: Copilot manifest explicitly exposes skills/ path'
        Assert-True -Condition ($copilotManifest.components.commands -eq 'commands/') -Label 'TP-005: Copilot manifest explicitly exposes commands/ path'
        Assert-True -Condition ($copilotManifest.components.mcpConfig -eq '.mcp.json') -Label 'TP-005: Copilot manifest explicitly exposes .mcp.json path'
        Assert-True -Condition (-not (Test-Path (Join-Path $canonicalPluginRoot 'agents\golem-reviewer.agent.md'))) -Label 'TP-005: Copilot-visible agents/ excludes AGY .agent.md duplicates'
        Assert-True -Condition (Test-Path (Join-Path $canonicalPluginRoot 'agy-agents\golem-reviewer.agent.md')) -Label 'TP-007: Superset canonical root contains AGY-specific agy-agents payload'
        Assert-True -Condition (Test-Path $agyRootPluginJsonPath) -Label 'TP-007: Superset canonical root contains AGY root plugin.json'
        Assert-True -Condition (Test-Path $agyMcpConfigPath) -Label 'TP-007: Superset canonical root contains AGY mcp_config.json'
        Assert-True -Condition (Test-Path $agyRulesPath) -Label 'TP-007: Superset canonical root contains AGY rules/gal.md'
        Assert-True -Condition ($pluginJson.canonicalPackage.packageId -eq 'gal-core') -Label 'TP-007: AGY root manifest preserves canonical package identity'
        Assert-True -Condition ($pluginJson.deferredCompanionPlugins.pluginId -contains 'dart-skills') -Label 'TP-007: AGY root manifest preserves deferred companion identity'
        Assert-True -Condition (($pluginJson.agents | Where-Object { $_.name -eq 'golem-reviewer' } | Select-Object -First 1).source -eq 'agy-agents/golem-reviewer.agent.md') -Label 'TP-007: AGY root manifest points agents to agy-agents/'

        $claudeManifest = Get-Content -LiteralPath $claudeManifestPath -Raw | ConvertFrom-Json

        Assert-True -Condition (Test-Path $claudeManifestPath) -Label 'TP-001: Claude artifact contains .claude-plugin/plugin.json'
        Assert-True -Condition (Test-Path $claudeSkillPath) -Label 'TP-003: Claude artifact contains skill payloads'
        Assert-True -Condition (Test-Path $claudeCommandPath) -Label 'TP-003: Claude artifact contains command markdown'
        Assert-True -Condition (Test-Path $claudeAgentPath) -Label 'TP-003: Claude artifact contains agent markdown'
        Assert-True -Condition (Test-Path $claudeMcpPath) -Label 'TP-003: Claude artifact contains plugin-root MCP config'
        Assert-True -Condition ($claudeManifest.name -eq 'gal') -Label 'TP-002: Claude manifest preserves plugin identity'
        Assert-True -Condition ($claudeManifest.displayName -eq 'Golem Agents Legion') -Label 'TP-002: Claude manifest preserves plugin display name'

        $rerunMarker = 'T-013 agy rerun propagation marker'
        Add-Content -LiteralPath (Join-Path $mutableRepoRoot 'plugins\gal-core\skills\defuddle\SKILL.md') -Value "`n$rerunMarker`n" -Encoding utf8

        & (Join-Path $PSScriptRoot 'Build-ProviderPlugins.ps1') -RepoRoot $mutableRepoRoot -ConfigPath $configPath -LockfilePath $lockfilePath -Providers @('agy') | Out-Null
        $agyLedgerAfterRerun = Get-Content -LiteralPath $agyLedgerPath -Raw | ConvertFrom-Json

        Assert-True -Condition (Test-LinkTargetsCanonical -Path (Get-AgyPluginInstallTarget) -ExpectedTarget $canonicalPluginRoot) -Label 'TP-014: AGY CLI junction survives rerender and still targets canonical root'
        Assert-True -Condition (Test-LinkTargetsCanonical -Path $agyIdeTarget -ExpectedTarget $canonicalPluginRoot) -Label 'TP-014: AGY IDE junction survives rerender and still targets canonical root'
        Assert-True -Condition (Test-LinkTargetsCanonical -Path $agyGuiConfigTarget -ExpectedTarget $canonicalPluginRoot) -Label 'TP-014: AGY GUI-config junction survives rerender and still targets canonical root'
        Assert-True -Condition ($agyLedgerAfterRerun.status -eq 'linked-projection' -and $agyLedgerAfterRerun.readSurface -eq 'linked-projection') -Label 'TP-014: AGY ledger stays linked-projection after rerender'
        Assert-True -Condition ((Get-Content -LiteralPath (Join-Path $canonicalPluginRoot 'skills\defuddle\SKILL.md') -Raw).Contains($rerunMarker)) -Label 'TP-010: canonical plugin content reflects the edited source after AGY rerun without -Force'
        Assert-True -Condition ((Get-Content -LiteralPath (Join-Path (Get-AgyPluginInstallTarget) 'skills\defuddle\SKILL.md') -Raw).Contains($rerunMarker)) -Label 'TP-010: AGY linked projection reflects the edited source after rerun without -Force'
    }
    finally {
        if (Test-Path $resolvedJsonPath) {
            Remove-Item -LiteralPath $resolvedJsonPath -Force
        }
    }
}
finally {
    $env:USERPROFILE = $originalUserProfile
    $env:HOME = $originalHome
    if (Test-Path $testHome) {
        Remove-Item -LiteralPath $testHome -Recurse -Force
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