<#
.SYNOPSIS
    Orchestrates GAL machine setup by invoking concern-specific update scripts.

.DESCRIPTION
    `Setup-Machine.ps1` resolves runtime selection once, then runs:
      1. `Update-Personalization.ps1`
      2. `Update-Skills.ps1`
      3. `Update-Commands.ps1`
      4. `Update-Mcp.ps1`
            5. `Install-GalPlugins.ps1`

    Each update script can also run standalone. `mcp.json` plus optional
        `mcp.local.json` remains the MCP source of truth, while Copilot MCP is
        written separately for VS Code Copilot and Copilot CLI.

.PARAMETER Uninstall
    Remove GAL-managed links and generated artifacts where supported.

.PARAMETER Replace
    When a real item already exists at the target, rename it to `.bak` before
    creating the GAL-managed link.

.PARAMETER DryRun
    Show what would happen without making changes.

.PARAMETER Reconfigure
    Prompt again for selected runtimes and primary runtime before invoking the
    update scripts.

.EXAMPLE
    .\scripts\Setup-Machine.ps1
    .\scripts\Setup-Machine.ps1 -Replace
    .\scripts\Setup-Machine.ps1 -DryRun
    .\scripts\Setup-Machine.ps1 -Reconfigure
    .\scripts\Setup-Machine.ps1 -Uninstall
#>
#Requires -Version 5.1
param(
    [switch]$Uninstall,
    [switch]$Replace,
    [switch]$DryRun,
    [switch]$Reconfigure,
    [string[]]$SelectedRuntimes,
    [string]$PrimaryRuntime
)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'common\Common.ps1')

$context = Initialize-SetupSession -EntryScriptPath $MyInvocation.MyCommand.Path -Uninstall:$Uninstall -Replace:$Replace -DryRun:$DryRun -Reconfigure:$Reconfigure -SelectedRuntimes $SelectedRuntimes -PrimaryRuntime $PrimaryRuntime -EnsureRipgrep

# --- AGY legacy pre-cleanup ---
# Remove all GAL-managed AGY legacy surfaces before any concern script runs.
# This ensures a clean slate for the plugin-only install path.
if ($context.InstallAntigravity -and -not $Uninstall) {
    Write-Host ''
    Write-Host '=== AGY legacy pre-cleanup ==='
    $legacyPaths = @(
        @{ Path = $context.AntigravitySkillsTarget; Label = 'legacy AGY skills directory'; Type = 'Directory' },
        @{ Path = $context.GalRootAntigravity; Label = 'legacy AGY GAL_ROOT symlink'; Type = 'Link' },
        @{ Path = $context.AgyPluginInstallTarget; Label = 'existing AGY plugin install'; Type = 'Directory' }
    )
    foreach ($legacy in $legacyPaths) {
        if (Test-Path $legacy.Path) {
            if ($DryRun) {
                Write-Host "  [DRY RUN] Would remove $($legacy.Label): $($legacy.Path)"
            }
            else {
                Remove-Item -LiteralPath $legacy.Path -Recurse -Force
                Write-Host "  [REMOVED] $($legacy.Label): $($legacy.Path)"
            }
        }
    }
    # Clean GAL-managed entries from global AGY mcp_config.json
    $globalMcpFile = $context.AntigravityMcpFile
    if (Test-Path $globalMcpFile) {
        $rawJson = Get-Content $globalMcpFile -Raw -Encoding UTF8
        try {
            $mcpConfig = $rawJson | ConvertFrom-Json
            $serversProp = Get-Member -InputObject $mcpConfig -Name 'mcpServers' -MemberType NoteProperty
            if ($null -ne $serversProp) {
                $servers = $mcpConfig.mcpServers
                $galKeys = @($servers.PSObject.Properties.Name) | Where-Object {
                    $_ -match '^gal-' -or $_ -eq 'gal'
                }
                if ($galKeys.Count -gt 0) {
                    if ($DryRun) {
                        Write-Host "  [DRY RUN] Would remove GAL-managed MCP entries from: $globalMcpFile ($($galKeys -join ', '))"
                    }
                    else {
                        foreach ($key in $galKeys) {
                            $servers.PSObject.Properties.Remove($key)
                        }
                        [System.IO.File]::WriteAllText($globalMcpFile, ($mcpConfig | ConvertTo-Json -Depth 10), $context.Utf8NoBom)
                        Write-Host "  [REMOVED] GAL-managed MCP entries from: $globalMcpFile ($($galKeys -join ', '))"
                    }
                }
            }
        }
        catch {
            Write-Host "  [WARN] Could not parse $globalMcpFile for legacy cleanup — skipping" -ForegroundColor Yellow
        }
    }
}

$sharedArguments = @{
    Uninstall = $Uninstall
    Replace = $Replace
    DryRun = $DryRun
    Reconfigure = $Reconfigure
}
if (-not $Uninstall) {
    $sharedArguments['SelectedRuntimes'] = @($context.SelectedRuntimes)
    $sharedArguments['PrimaryRuntime'] = $context.PrimaryRuntime
}

$steps = @(
    [pscustomobject]@{ Name = 'Personalization'; Path = Join-Path $PSScriptRoot 'Update-Personalization.ps1' },
    [pscustomobject]@{ Name = 'Skills'; Path = Join-Path $PSScriptRoot 'Update-Skills.ps1' },
    [pscustomobject]@{ Name = 'Commands'; Path = Join-Path $PSScriptRoot 'Update-Commands.ps1' },
    [pscustomobject]@{ Name = 'MCP'; Path = Join-Path $PSScriptRoot 'Update-Mcp.ps1' },
    [pscustomobject]@{ Name = 'Install Orchestration'; Path = Join-Path $PSScriptRoot 'Install-GalPlugins.ps1' }
)

foreach ($step in $steps) {
    Write-Host ''
    Write-Host ('>>> Running {0}' -f $step.Name)
    & $step.Path @sharedArguments
}

Write-Host ''
if ($Uninstall) {
    Write-Host 'Uninstall complete.'
}
elseif ($DryRun) {
    Write-Host 'Dry run complete. No changes made.'
    Write-Host ('Selected runtimes: {0}' -f (@($context.SelectedRuntimes) -join ', '))
    Write-Host ('Primary runtime: {0}' -f $context.PrimaryRuntime)
}
else {
    Write-Host ('Setup complete: runtimes={0}; primary={1}' -f (@($context.SelectedRuntimes) -join ', '), $context.PrimaryRuntime)
    Write-Host 'Note: If SKILL.template.md or SKILL.local.md changes, rerun Update-Commands.ps1 or Setup-Machine.ps1.'
}