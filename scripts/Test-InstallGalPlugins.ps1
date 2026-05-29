#Requires -Version 5.1

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Assert-Contains {
    param(
        [string]$Text,
        [string]$Expected,
        [string]$Message
    )

    if (-not $Text.Contains($Expected)) {
        throw "$Message`nExpected to find: $Expected`nActual output:`n$Text"
    }
}

function Assert-NotContains {
    param(
        [string]$Text,
        [string]$Unexpected,
        [string]$Message
    )

    if ($Text.Contains($Unexpected)) {
        throw "$Message`nDid not expect to find: $Unexpected`nActual output:`n$Text"
    }
}

function Assert-Throws {
    param(
        [scriptblock]$Action,
        [string]$Expected,
        [string]$Message
    )

    try {
        & $Action
    }
    catch {
        if ($_.Exception.Message -like "*$Expected*") {
            return
        }

        throw "$Message`nExpected exception containing: $Expected`nActual exception:`n$($_.Exception.Message)"
    }

    throw "$Message`nExpected exception containing: $Expected`nActual result: no exception"
}

function New-TestDirectoryLink {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string]$Target
    )

    try {
        New-Item -ItemType SymbolicLink -Path $Path -Target $Target | Out-Null
    }
    catch {
        New-Item -ItemType Junction -Path $Path -Target $Target | Out-Null
    }
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$scriptUnderTest = Join-Path $repoRoot 'scripts\Install-GalPlugins.ps1'
$setupScriptUnderTest = Join-Path $repoRoot 'scripts\Setup-Machine.ps1'
$uninstallScriptUnderTest = Join-Path $repoRoot 'scripts\Uninstall-Machine.ps1'
$providerBuildScriptUnderTest = Join-Path $repoRoot 'scripts\Build-ProviderPlugins.ps1'
$updateSkillsScriptUnderTest = Join-Path $repoRoot 'scripts\Update-Skills.ps1'
$updateCommandsScriptUnderTest = Join-Path $repoRoot 'scripts\Update-Commands.ps1'
$testHome = Join-Path $env:TEMP ("gal-install-test-{0}" -f [System.Guid]::NewGuid().ToString('N'))
$originalUserProfile = $env:USERPROFILE
$originalHome = $env:HOME

New-Item -ItemType Directory -Path $testHome | Out-Null

try {
    $env:USERPROFILE = $testHome
    $env:HOME = $testHome

    $galRoot = Join-Path $testHome '.gal'
    New-Item -ItemType Directory -Path $galRoot | Out-Null

    $installStatePath = Join-Path $galRoot 'install-state.json'
    $installState = [ordered]@{
        selectedRuntimes = @('copilot', 'antigravity', 'codex', 'opencode')
        primaryRuntime = 'copilot'
    }
    $installState | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $installStatePath -Encoding utf8

    $sourceOutput = (& $scriptUnderTest -RepoRoot $repoRoot -SelectedRuntimes @('copilot', 'antigravity', 'codex', 'opencode') -PrimaryRuntime 'copilot' -DryRun 6>&1 | Out-String)
    Assert-Contains $sourceOutput '[DRY RUN] Would seed machine config:' 'Source-mode dry run should preview config seeding.'
    Assert-Contains $sourceOutput '[OK] Mode: source' 'Source-mode dry run should report source mode.'
    Assert-Contains $sourceOutput '[OK] Primary provider lanes: copilot, agy, codex' 'Source-mode dry run should surface resolved primary providers.'
    Assert-Contains $sourceOutput '[OK] Bridge lanes: opencode' 'Source-mode dry run should surface bridge lanes.'

    $machineConfigPath = Join-Path $testHome '.gal\config\config.json'
    $pluginsLockPath = Join-Path $testHome '.gal\state\plugins.lock.json'
    if (Test-Path $machineConfigPath) {
        throw "Dry run should not write machine config: $machineConfigPath"
    }
    if (Test-Path $pluginsLockPath) {
        throw "Dry run should not write the real plugins lockfile: $pluginsLockPath"
    }

    $configRoot = Split-Path -Parent $machineConfigPath
    New-Item -ItemType Directory -Path $configRoot -Force | Out-Null
    $installConfig = [ordered]@{
        schemaVersion = 1
        galRoot = $repoRoot
        devMode = $false
        defaultProfile = 'default'
        profiles = [ordered]@{}
        enabledPlugins = @()
        disabledPlugins = @()
        providerSelections = [ordered]@{
            copilot = [ordered]@{ enabled = $true; lane = 'primary' }
            agy = [ordered]@{ enabled = $true; lane = 'primary' }
            codex = [ordered]@{ enabled = $false; lane = 'primary' }
            opencode = [ordered]@{ enabled = $true; lane = 'bridge' }
        }
        installMode = 'install'
        preferredProviders = @('copilot', 'agy')
        userSettings = [ordered]@{
            upgradeSentinel = 'preserve-me'
        }
    }
    $installConfig | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $machineConfigPath -Encoding utf8

    $xmachineConfigPath = Join-Path $testHome '.gal\config\xmachine.json'
    $xmachineConfig = [ordered]@{
        schemaVersion = 1
        defaultNode = 'test-node'
    }
    $xmachineConfig | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $xmachineConfigPath -Encoding utf8

    $existingLockfileRoot = Split-Path -Parent $pluginsLockPath
    New-Item -ItemType Directory -Path $existingLockfileRoot -Force | Out-Null
    $existingLockfile = [ordered]@{
        schemaVersion = 1
        plugins = @(
            [ordered]@{
                name = 'gal-core'
                version = 'v-test'
            }
        )
    }
    $existingLockfile | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $pluginsLockPath -Encoding utf8

    $machineConfigBeforeUpgrade = Get-Content -LiteralPath $machineConfigPath -Raw -Encoding utf8
    $xmachineConfigBeforeUpgrade = Get-Content -LiteralPath $xmachineConfigPath -Raw -Encoding utf8
    $lockfileBeforeUpgrade = Get-Content -LiteralPath $pluginsLockPath -Raw -Encoding utf8

    $installOutput = (& $scriptUnderTest -RepoRoot $repoRoot -ConfigPath $machineConfigPath -LockfilePath $pluginsLockPath -SelectedRuntimes @('copilot', 'antigravity', 'codex', 'opencode') -PrimaryRuntime 'copilot' -DryRun 6>&1 | Out-String)
    Assert-Contains $installOutput '[OK] Mode: install' 'Install-mode dry run should report install mode.'
    Assert-Contains $installOutput '[OK] Install mode disables repo-root links and source-only local overrides.' 'Install-mode dry run should declare repo-root links disabled.'
    Assert-Contains $installOutput '[agy] mode=managed-shortcut renderer=Build-AgyPlugin.ps1 shortcut=' 'Install-mode dry run should surface AGY provider orchestration.'
    Assert-Contains $installOutput '[copilot] mode=native-install renderer=not-yet-implemented shortcut=none' 'Install-mode dry run should surface Copilot provider orchestration.'

    $setupInstallOutput = (& $setupScriptUnderTest -SelectedRuntimes @('copilot', 'antigravity', 'codex', 'opencode') -PrimaryRuntime 'copilot' -DryRun 6>&1 | Out-String)
    Assert-Contains $setupInstallOutput '>>> Skipping Skills' 'Install-mode setup should skip source-only skills wiring.'
    Assert-Contains $setupInstallOutput '>>> Skipping Commands' 'Install-mode setup should skip source-only command baking.'
    Assert-Contains $setupInstallOutput '[OK] Mode: install' 'Install-mode setup should still run install orchestration.'
    Assert-Contains $setupInstallOutput 'Install mode delegates AGY plugin lifecycle to Install-GalPlugins.ps1.' 'Install-mode setup should not install AGY through legacy concern scripts.'
    Assert-NotContains $setupInstallOutput '=== GAL_ROOT symlinks ===' 'Install-mode setup should not build repo-root GAL_ROOT symlinks.'

    $machineConfigAfterUpgrade = Get-Content -LiteralPath $machineConfigPath -Raw -Encoding utf8
    if ($machineConfigAfterUpgrade -ne $machineConfigBeforeUpgrade) {
        throw "Install-mode dry run should preserve existing machine config during upgrade-style refreshes."
    }

    $xmachineConfigAfterUpgrade = Get-Content -LiteralPath $xmachineConfigPath -Raw -Encoding utf8
    if ($xmachineConfigAfterUpgrade -ne $xmachineConfigBeforeUpgrade) {
        throw "Install-mode dry run should preserve xmachine bindings during upgrade-style refreshes."
    }

    $lockfileAfterUpgrade = Get-Content -LiteralPath $pluginsLockPath -Raw -Encoding utf8
    if ($lockfileAfterUpgrade -ne $lockfileBeforeUpgrade) {
        throw "Install-mode dry run should preserve the existing lockfile during upgrade-style refreshes."
    }

    if (Test-Path $pluginsLockPath) {
        Assert-Contains $lockfileAfterUpgrade 'gal-core' 'Install-mode dry run should leave the existing lockfile content intact.'
    }

    $activeRoot = Join-Path $testHome '.gal\active'
    New-Item -ItemType Directory -Path $activeRoot -Force | Out-Null
    $agyInstallTarget = Join-Path $testHome '.gemini\antigravity-cli\plugins\gal'
    New-Item -ItemType Directory -Path $agyInstallTarget -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $agyInstallTarget 'plugin.json') -Value '{}' -Encoding utf8
    $agyShortcut = Join-Path $activeRoot 'agy'
    New-TestDirectoryLink -Path $agyShortcut -Target $agyInstallTarget

    $managedPluginStore = Join-Path $testHome '.gal\plugins'
    $managedMcpRoot = Join-Path $testHome '.gal\generated\mcp'
    $managedXmachineRoot = Join-Path $testHome '.gal\generated\xmachine'
    $managedProvidersRoot = Join-Path $testHome '.gal\dist\providers'
    $claudeLifecycleStatePath = Join-Path $managedProvidersRoot 'claude\managed.json'
    New-Item -ItemType Directory -Path $managedPluginStore -Force | Out-Null
    New-Item -ItemType Directory -Path $managedMcpRoot -Force | Out-Null
    New-Item -ItemType Directory -Path $managedXmachineRoot -Force | Out-Null
    New-Item -ItemType Directory -Path $managedProvidersRoot -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $managedPluginStore 'managed.txt') -Value 'gal-managed' -Encoding utf8
    Set-Content -LiteralPath (Join-Path $managedMcpRoot 'managed.json') -Value '{}' -Encoding utf8
    Set-Content -LiteralPath (Join-Path $managedXmachineRoot 'managed.json') -Value '{}' -Encoding utf8
    New-Item -ItemType Directory -Path (Split-Path -Parent $claudeLifecycleStatePath) -Force | Out-Null
    Set-Content -LiteralPath $claudeLifecycleStatePath -Value '{}' -Encoding utf8

    $machineConfigBeforeUninstall = Get-Content -LiteralPath $machineConfigPath -Raw -Encoding utf8
    $uninstallOutput = (& $uninstallScriptUnderTest -DryRun 6>&1 | Out-String)
    Assert-Contains $uninstallOutput '[OK] Install-mode uninstall owns AGY and Claude provider-lifecycle metadata cleanup.' 'Install-mode uninstall should be owned by install orchestration.'
    Assert-Contains $uninstallOutput '[DRY RUN] Would remove AGY plugin install target:' 'Install-mode uninstall should preview AGY plugin removal.'
    Assert-Contains $uninstallOutput '[DRY RUN] Would remove GAL canonical plugin root:' 'Install-mode uninstall should preview canonical plugin-root removal.'
    Assert-Contains $uninstallOutput '[DRY RUN] Would remove GAL-managed MCP projections:' 'Install-mode uninstall should preview GAL-managed MCP projection removal.'
    Assert-Contains $uninstallOutput '[DRY RUN] Would remove GAL-managed xmachine projections:' 'Install-mode uninstall should preview GAL-managed xmachine projection removal.'
    Assert-Contains $uninstallOutput '[DRY RUN] Would remove GAL-managed provider projections:' 'Install-mode uninstall should preview GAL-managed provider projection removal.'

    $legacyClaudeSkillsRoot = Join-Path $testHome '.claude\skills'
    $legacyClaudeCommandsRoot = Join-Path $testHome '.claude\commands'
    New-Item -ItemType Directory -Path $legacyClaudeSkillsRoot -Force | Out-Null
    New-Item -ItemType Directory -Path $legacyClaudeCommandsRoot -Force | Out-Null
    $legacyClaudeSkillLink = Join-Path $legacyClaudeSkillsRoot 'defuddle'
    New-TestDirectoryLink -Path $legacyClaudeSkillLink -Target (Join-Path $repoRoot 'skills\defuddle')
    Set-Content -LiteralPath (Join-Path $legacyClaudeCommandsRoot 'gal.md') -Value '# Generated by GAL Setup-Machine. Do not edit manually.' -Encoding utf8

    $updateSkillsOutput = (& $updateSkillsScriptUnderTest -SelectedRuntimes @('claude') -PrimaryRuntime 'claude' -DryRun 6>&1 | Out-String)
    Assert-Contains $updateSkillsOutput '=== Claude legacy skills cleanup' 'Claude source-mode skills flow should only perform legacy cleanup.'
    Assert-Contains $updateSkillsOutput '.claude\skills\defuddle' 'Claude source-mode skills cleanup should target legacy Claude skill links.'
    Assert-NotContains $updateSkillsOutput '=== Claude Skills (' 'Claude source-mode skills flow should no longer create Claude skill links.'

    $updateCommandsOutput = (& $updateCommandsScriptUnderTest -SelectedRuntimes @('claude') -PrimaryRuntime 'claude' -DryRun 6>&1 | Out-String)
    Assert-Contains $updateCommandsOutput '=== Claude legacy command cleanup ===' 'Claude source-mode command flow should only perform legacy cleanup.'
    Assert-Contains $updateCommandsOutput '.claude\commands\gal.md' 'Claude source-mode command cleanup should target legacy Claude command files.'
    Assert-NotContains $updateCommandsOutput '=== Claude custom commands ===' 'Claude source-mode command flow should no longer create Claude command files.'

    $claudeConfig = [ordered]@{
        schemaVersion = 1
        galRoot = $repoRoot
        devMode = $false
        defaultProfile = 'default'
        profiles = [ordered]@{}
        enabledPlugins = @()
        disabledPlugins = @()
        providerSelections = [ordered]@{
            claude = [ordered]@{ enabled = $true; lane = 'primary' }
        }
        installMode = 'install'
        preferredProviders = @('claude')
        userSettings = [ordered]@{}
    }
    $claudeConfig | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $machineConfigPath -Encoding utf8

    $claudeLifecycleOutput = (& $scriptUnderTest -RepoRoot $repoRoot -ConfigPath $machineConfigPath -LockfilePath $pluginsLockPath -SelectedRuntimes @('claude') -PrimaryRuntime 'claude' -Force 6>&1 | Out-String)
    Assert-Contains $claudeLifecycleOutput '[OK] Evaluating Claude plugin lifecycle.' 'Claude install orchestration should enter the Claude lifecycle path.'
    Assert-Contains $claudeLifecycleOutput '[OK] Wrote Claude lifecycle state:' 'Claude install orchestration should persist Claude lifecycle state.'
    if (Get-Command claude -ErrorAction SilentlyContinue) {
        Assert-Contains $claudeLifecycleOutput '[OK] Claude plugin validation passed.' 'Claude lifecycle test should validate the artifact when Claude CLI is available.'
        Assert-Contains $claudeLifecycleOutput '[OK] Session smoke command: claude --plugin-dir' 'Claude lifecycle test should record the session-load fallback when artifact install is unavailable.'
    }

    if (-not (Test-Path $claudeLifecycleStatePath)) {
        throw "Claude lifecycle install should write lifecycle state to $claudeLifecycleStatePath"
    }

    $claudeLifecycleState = Get-Content -LiteralPath $claudeLifecycleStatePath -Raw -Encoding utf8 | ConvertFrom-Json
    if ($claudeLifecycleState.provider -ne 'claude') {
        throw 'Claude lifecycle state should record provider=claude.'
    }
    if ($claudeLifecycleState.canonicalRoot -ne (Join-Path $testHome '.gal\plugins\gal')) {
        throw 'Claude lifecycle state should record the canonical root under ~/.gal/plugins/gal.'
    }
    if ($claudeLifecycleState.packageOutputRoot -ne (Join-Path $testHome '.gal\dist\provider-plugins\claude\gal')) {
        throw 'Claude lifecycle state should record the package output root under ~/.gal/dist/provider-plugins/claude/gal.'
    }
    if ($claudeLifecycleState.projectionRoot -ne (Join-Path $testHome '.claude\plugins\gal')) {
        throw 'Claude lifecycle state should record the provider-visible projection root.'
    }
    if ($claudeLifecycleState.lifecycle.mode -ne 'session-load-only') {
        throw "Claude lifecycle state should record session-load-only mode for the current CLI capability. Actual: $($claudeLifecycleState.lifecycle.mode)"
    }
    if ($claudeLifecycleState.cli.available -and $claudeLifecycleState.cli.validateSupported -and -not $claudeLifecycleState.validation.strictPassed) {
        throw 'Claude lifecycle state should mark strict validation passed when Claude CLI validation is available.'
    }

    $machineConfigBeforeClaudeUninstall = Get-Content -LiteralPath $machineConfigPath -Raw -Encoding utf8
    $xmachineConfigBeforeClaudeUninstall = Get-Content -LiteralPath $xmachineConfigPath -Raw -Encoding utf8
    $lockfileBeforeClaudeUninstall = Get-Content -LiteralPath $pluginsLockPath -Raw -Encoding utf8

    $machineConfigAfterUninstall = Get-Content -LiteralPath $machineConfigPath -Raw -Encoding utf8
    if ($machineConfigAfterUninstall -ne $machineConfigBeforeClaudeUninstall) {
        throw "Install-mode uninstall dry run should preserve existing machine config."
    }

    $xmachineConfigAfterUninstall = Get-Content -LiteralPath $xmachineConfigPath -Raw -Encoding utf8
    if ($xmachineConfigAfterUninstall -ne $xmachineConfigBeforeClaudeUninstall) {
        throw "Install-mode uninstall dry run should preserve xmachine bindings."
    }

    $lockfileAfterUninstall = Get-Content -LiteralPath $pluginsLockPath -Raw -Encoding utf8
    if ($lockfileAfterUninstall -ne $lockfileBeforeClaudeUninstall) {
        throw "Install-mode uninstall dry run should preserve the existing lockfile."
    }

    $purgeOutput = (& $uninstallScriptUnderTest -DryRun -Purge 6>&1 | Out-String)
    Assert-Contains $purgeOutput '[OK] Explicit purge requested; removing preserved machine-local state.' 'Explicit purge should be opt-in and visible.'
    Assert-Contains $purgeOutput '[DRY RUN] Would remove GAL config root:' 'Explicit purge should preview config-root deletion.'
    Assert-Contains $purgeOutput '[DRY RUN] Would remove GAL state directory:' 'Explicit purge should preview state-directory deletion.'
    Assert-Contains $purgeOutput '[DRY RUN] Would remove GAL install-state file:' 'Explicit purge should preview install-state deletion.'
    Assert-Throws -Expected 'Explicit purge requires -ConfirmPurge' -Message 'Destructive purge should require an explicit confirmation switch.' -Action {
        & $uninstallScriptUnderTest -Purge 6>&1 | Out-Null
    }

    Remove-Item -LiteralPath $machineConfigPath -Force
    $fallbackUninstallOutput = (& $scriptUnderTest -RepoRoot $repoRoot -LockfilePath $pluginsLockPath -DryRun -Uninstall 6>&1 | Out-String)
    Assert-Contains $fallbackUninstallOutput '[OK] Falling back to install-mode uninstall because GAL-managed runtime artifacts are present.' 'Missing machine config should still allow managed uninstall cleanup.'
    Assert-NotContains $fallbackUninstallOutput '[SKIP] Source-mode uninstall remains owned by the legacy concern scripts.' 'Missing machine config should not route uninstall back through the source-mode skip path.'
    Set-Content -LiteralPath $machineConfigPath -Value $machineConfigBeforeUninstall -Encoding utf8

    & $providerBuildScriptUnderTest -RepoRoot $repoRoot -ConfigPath $machineConfigPath -LockfilePath $pluginsLockPath -Providers @('agy') -Force 6>&1 | Out-Null
    if (-not (Test-Path $agyInstallTarget)) {
        throw "AGY provider build should install into $agyInstallTarget"
    }
    if (-not (Test-Path $agyShortcut)) {
        throw "AGY provider build should create stable shortcut at $agyShortcut"
    }

    Remove-Item -LiteralPath $agyShortcut -Force
    New-Item -ItemType Directory -Path $agyShortcut | Out-Null
    Assert-Throws -Expected 'Failed to claim GAL-managed shortcut for provider' -Message 'AGY provider build should fail when the managed shortcut path is occupied by a non-link item.' -Action {
        & $providerBuildScriptUnderTest -RepoRoot $repoRoot -ConfigPath $machineConfigPath -LockfilePath $pluginsLockPath -Providers @('agy') -Force 6>&1 | Out-Null
    }

    Remove-Item -LiteralPath $agyShortcut -Recurse -Force
    & $providerBuildScriptUnderTest -RepoRoot $repoRoot -ConfigPath $machineConfigPath -LockfilePath $pluginsLockPath -Providers @('agy') -Force 6>&1 | Out-Null

    & $scriptUnderTest -RepoRoot $repoRoot -ConfigPath $machineConfigPath -LockfilePath $pluginsLockPath -Uninstall 6>&1 | Out-Null
    if (Test-Path $agyShortcut) {
        throw "Install-mode uninstall should remove the GAL-managed AGY shortcut."
    }
    if (Test-Path $agyInstallTarget) {
        throw "Install-mode uninstall should remove the AGY provider install target."
    }
    if (Test-Path $managedPluginStore) {
        throw "Install-mode uninstall should remove the GAL canonical plugin root."
    }
    if (Test-Path $managedMcpRoot) {
        throw "Install-mode uninstall should remove GAL-managed MCP projections."
    }
    if (Test-Path $managedXmachineRoot) {
        throw "Install-mode uninstall should remove GAL-managed xmachine projections."
    }
    if (Test-Path $managedProvidersRoot) {
        throw "Install-mode uninstall should remove GAL-managed provider projections."
    }
    if (-not (Test-Path $machineConfigPath)) {
        throw "Install-mode uninstall should preserve machine config by default."
    }
    if (-not (Test-Path $xmachineConfigPath)) {
        throw "Install-mode uninstall should preserve xmachine bindings by default."
    }
    if (-not (Test-Path $pluginsLockPath)) {
        throw "Install-mode uninstall should preserve the lockfile by default."
    }

    New-Item -ItemType Directory -Path $activeRoot -Force | Out-Null
    $brokenAgyTarget = Join-Path $testHome '.gemini\antigravity-cli\plugins\missing-gal'
    New-Item -ItemType Directory -Path $brokenAgyTarget -Force | Out-Null
    New-TestDirectoryLink -Path $agyShortcut -Target $brokenAgyTarget
    Remove-Item -LiteralPath $brokenAgyTarget -Recurse -Force
    & $scriptUnderTest -RepoRoot $repoRoot -ConfigPath $machineConfigPath -LockfilePath $pluginsLockPath -Uninstall 6>&1 | Out-Null
    if ($null -ne (Get-Item -LiteralPath $agyShortcut -Force -ErrorAction SilentlyContinue)) {
        throw "Install-mode uninstall should remove broken GAL-managed AGY shortcuts."
    }

    New-Item -ItemType Directory -Path $activeRoot -Force | Out-Null
    New-Item -ItemType Directory -Path $brokenAgyTarget -Force | Out-Null
    New-TestDirectoryLink -Path $agyShortcut -Target $brokenAgyTarget
    Remove-Item -LiteralPath $brokenAgyTarget -Recurse -Force
    Remove-Item -LiteralPath $machineConfigPath -Force
    Remove-Item -LiteralPath $installStatePath -Force
    $brokenLinkFallbackOutput = (& $scriptUnderTest -RepoRoot $repoRoot -LockfilePath $pluginsLockPath -DryRun -Uninstall 6>&1 | Out-String)
    Assert-Contains $brokenLinkFallbackOutput '[OK] Falling back to install-mode uninstall because GAL-managed runtime artifacts are present.' 'Broken GAL-managed shortcuts should still count as install-owned uninstall evidence.'
    Assert-NotContains $brokenLinkFallbackOutput '[SKIP] Source-mode uninstall remains owned by the legacy concern scripts.' 'Broken GAL-managed shortcuts should not be ignored by the uninstall ownership probe.'

    Write-Host 'PASS: Install orchestration dry-run behavior verified.'
}
finally {
    $env:USERPROFILE = $originalUserProfile
    $env:HOME = $originalHome

    if (Test-Path $testHome) {
        Remove-Item -LiteralPath $testHome -Recurse -Force
    }
}
