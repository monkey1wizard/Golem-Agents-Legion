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

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$scriptUnderTest = Join-Path $repoRoot 'scripts\Install-GalPlugins.ps1'
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
        userSettings = [ordered]@{}
    }
    $installConfig | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $machineConfigPath -Encoding utf8

    $installOutput = (& $scriptUnderTest -RepoRoot $repoRoot -ConfigPath $machineConfigPath -LockfilePath $pluginsLockPath -SelectedRuntimes @('copilot', 'antigravity', 'codex', 'opencode') -PrimaryRuntime 'copilot' -DryRun 6>&1 | Out-String)
    Assert-Contains $installOutput '[OK] Mode: install' 'Install-mode dry run should report install mode.'
    Assert-Contains $installOutput '[agy] mode=managed-shortcut renderer=Build-AgyPlugin.ps1 shortcut=' 'Install-mode dry run should surface AGY provider orchestration.'
    Assert-Contains $installOutput '[copilot] mode=native-install renderer=not-yet-implemented shortcut=none' 'Install-mode dry run should surface Copilot provider orchestration.'

    if (Test-Path $pluginsLockPath) {
        throw "Install-mode dry run should not write the real plugins lockfile: $pluginsLockPath"
    }

    Write-Host 'PASS: Install orchestration dry-run behavior verified.'
}
finally {
    $env:USERPROFILE = $originalUserProfile
    $env:HOME = $originalHome

    if (Test-Path $testHome) {
        Remove-Item -LiteralPath $testHome -Recurse -Force
    }
}