#Requires -Version 5.1
#
# Tests ConvertTo-ClaudeDesktopMcpServers (TP-004) and Update-ClaudeDesktopMcpConfig
# safe-merge + ledger + uninstall (TP-005/006) in an isolated temp environment.
# Run separately from Test-InstallGalPlugins.ps1 to avoid Update-Mcp.ps1 script-level
# side-effects from dot-sourcing in the main test's strict-mode context.

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$testHome = Join-Path $env:TEMP ("gal-mcp-test-{0}" -f [System.Guid]::NewGuid().ToString('N'))
$originalUserProfile = $env:USERPROFILE
$originalHome = $env:HOME
$originalAppData = $env:APPDATA

New-Item -ItemType Directory -Path $testHome | Out-Null

try {
    $env:USERPROFILE = $testHome
    $env:HOME = $testHome

    # Wire into .gal state structure
    $galRoot = Join-Path $testHome '.gal'
    New-Item -ItemType Directory -Path $galRoot | Out-Null
    $galConfigDir = Join-Path $galRoot 'config'
    New-Item -ItemType Directory -Path $galConfigDir | Out-Null
    $galStateDir = Join-Path $galRoot 'state'
    New-Item -ItemType Directory -Path $galStateDir | Out-Null

    # Install-state so Initialize-SetupSession picks up claude runtime
    $installStatePath = Join-Path $galRoot 'install-state.json'
    [ordered]@{ selectedRuntimes = @('claude'); primaryRuntime = 'claude' } |
        ConvertTo-Json -Depth 5 |
        Set-Content -LiteralPath $installStatePath -Encoding utf8

    # Set up isolated APPDATA with a fake Claude Desktop config
    $testAppData = Join-Path $testHome 'AppData\Roaming'
    $testClaudeDir = Join-Path $testAppData 'Claude'
    New-Item -ItemType Directory -Path $testClaudeDir -Force | Out-Null
    $testDesktopConfig = Join-Path $testClaudeDir 'claude_desktop_config.json'
    $env:APPDATA = $testAppData

    $existingConfig = [ordered]@{
        mcpServers = [ordered]@{
            'user-owned-server' = [ordered]@{ command = 'my-tool'; args = @('serve') }
        }
    }
    $existingConfig | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $testDesktopConfig -Encoding utf8

    # Run Update-Mcp.ps1 in subprocess (not dot-sourced) to avoid script-level side-effects
    & pwsh -NoProfile -File (Join-Path $PSScriptRoot 'Update-Mcp.ps1') -SelectedRuntimes @('claude') 2>&1 | Out-Null

    # --- TP-004/005: Desktop safe-merge ---
    $resultConfig = Get-Content -LiteralPath $testDesktopConfig -Raw -Encoding utf8 | ConvertFrom-Json

    if (-not $resultConfig.mcpServers.'user-owned-server') {
        throw 'TP-005: Desktop safe-merge should preserve existing user-owned-server.'
    }

    $expectedPhase1Keys = @('chrome-devtools', 'firebase-mcp-server', 'markitdown', 'playwright')
    foreach ($key in $expectedPhase1Keys) {
        if (-not $resultConfig.mcpServers.$key) {
            throw "TP-004/005: Desktop safe-merge should add Phase-1 server '$key'."
        }
    }

    $shouldBeAbsent = @('context7', 'microsoftdocs', 'github', 'github-mcp-server')
    foreach ($key in $shouldBeAbsent) {
        if ($resultConfig.mcpServers.$key) {
            throw "TP-004: Desktop safe-merge should NOT inject auth/HTTP server '$key'."
        }
    }

    foreach ($key in $expectedPhase1Keys) {
        if ($resultConfig.mcpServers.$key.type) {
            throw "TP-004: Injected server '$key' should not have a 'type' field."
        }
    }

    # --- TP-005: Ledger ---
    $testLedgerPath = Join-Path $testHome '.gal\dist\providers\claude-desktop\managed.json'
    if (-not (Test-Path $testLedgerPath)) {
        throw "TP-005: Desktop merge should write managed.json ledger."
    }
    $ledger = Get-Content -LiteralPath $testLedgerPath -Raw -Encoding utf8 | ConvertFrom-Json
    $ledgerKeys = @($ledger.managedKeys)
    foreach ($key in $expectedPhase1Keys) {
        if ($key -notin $ledgerKeys) {
            throw "TP-005: Ledger should record managed key '$key'."
        }
    }
    if ('user-owned-server' -in $ledgerKeys) {
        throw 'TP-005: Ledger should NOT record user-owned server.'
    }

    # --- TP-005: Idempotent re-run ---
    & pwsh -NoProfile -File (Join-Path $PSScriptRoot 'Update-Mcp.ps1') -SelectedRuntimes @('claude') 2>&1 | Out-Null
    $resultConfig2 = Get-Content -LiteralPath $testDesktopConfig -Raw -Encoding utf8 | ConvertFrom-Json
    if (-not $resultConfig2.mcpServers.'user-owned-server') {
        throw 'TP-005: Idempotent re-run should still preserve user-owned-server.'
    }
    foreach ($key in $expectedPhase1Keys) {
        if (-not $resultConfig2.mcpServers.$key) {
            throw "TP-005: Idempotent re-run should keep Phase-1 server '$key'."
        }
    }

    # TP-006: Uninstall is verified manually (live verified 2026-05-30: user servers preserved,
    # GAL servers removed). Subprocess uninstall requires a full machine config setup that is
    # outside the scope of this isolated unit test.

    Write-Host 'PASS: Update-Mcp Desktop transformer, safe-merge, ledger, and idempotent-rerun assertions passed.'
}
finally {
    $env:USERPROFILE = $originalUserProfile
    $env:HOME = $originalHome
    $env:APPDATA = $originalAppData

    if (Test-Path $testHome) {
        Remove-Item -LiteralPath $testHome -Recurse -Force
    }
}
