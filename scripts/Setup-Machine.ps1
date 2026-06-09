<#
.SYNOPSIS
    Orchestrates GAL machine setup by invoking concern-specific update scripts.

.DESCRIPTION
    `Setup-Machine.ps1` resolves runtime selection once, then runs:
      1. `gal update --machine-only`
      2. `gal mcp update`
      3. `Install-GalPlugins.ps1`

    `mcp.json` plus optional
        `~/.gal/config/mcp.local.json` remains the MCP source of truth, while Copilot MCP is
        written separately for VS Code Copilot and Copilot CLI.

.PARAMETER Uninstall
    Remove GAL-managed links and generated artifacts where supported.

.PARAMETER Replace
    When a real item already exists at the target, rename it to `.bak` before
    creating the GAL-managed link.

.PARAMETER DryRun
    Show what would happen without making changes.

.PARAMETER Purge
    Only valid with `-Uninstall`. Also removes preserved machine-local state under `~/.gal/`.

.PARAMETER ConfirmPurge
    Required together with `-Purge` for destructive execution. Not needed with `-DryRun`.

.PARAMETER Reconfigure
    Prompt again for selected runtimes and primary runtime before invoking the
    update scripts.

.EXAMPLE
    .\scripts\Setup-Machine.ps1
    .\scripts\Setup-Machine.ps1 -Replace
    .\scripts\Setup-Machine.ps1 -DryRun
    .\scripts\Setup-Machine.ps1 -Check
    .\scripts\Setup-Machine.ps1 -Reconfigure
    .\scripts\Setup-Machine.ps1 -Uninstall
    .\scripts\Setup-Machine.ps1 -Uninstall -Purge -DryRun
    .\scripts\Setup-Machine.ps1 -Uninstall -Purge -ConfirmPurge
#>
#Requires -Version 5.1
param(
    [switch]$Uninstall,
    [switch]$Purge,
    [switch]$ConfirmPurge,
    [switch]$Replace,
    [switch]$DryRun,
    [switch]$Check,
    [switch]$Reconfigure,
    [switch]$BootstrapInstall,
    [string[]]$SelectedRuntimes,
    [string]$PrimaryRuntime
)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'common\Common.ps1')
$galExe = (Get-Command 'gal.exe' -ErrorAction SilentlyContinue | Select-Object -First 1).Source
if ([string]::IsNullOrWhiteSpace($galExe)) {
    throw 'gal.exe not found on PATH.'
}

function Get-ConfiguredInstallMode {
    param(
        [pscustomobject]$Context,
        [switch]$BootstrapInstall
    )

    if (-not (Test-Path $Context.GalConfigFile)) {
        if ($BootstrapInstall) {
            return 'install'
        }

        return 'source'
    }

    # Single authority: delegate to the shared devMode + galRoot resolver.
    # Do not read installMode here (deprecated; see Common.ps1 / OQ-003).
    return Get-ConfiguredInstallModeFromContext -Context $Context
}

$previousBootstrapEnv = $env:GAL_BOOTSTRAP_INSTALL
if ($BootstrapInstall) {
    $env:GAL_BOOTSTRAP_INSTALL = '1'
}

$context = Initialize-SetupSession -EntryScriptPath $MyInvocation.MyCommand.Path -Uninstall:$Uninstall -Replace:$Replace -DryRun:$DryRun -Reconfigure:$Reconfigure -SelectedRuntimes $SelectedRuntimes -PrimaryRuntime $PrimaryRuntime -EnsureRipgrep:(!$Check)
$context | Add-Member -NotePropertyName BootstrapInstall -NotePropertyValue $BootstrapInstall.IsPresent -Force
$installMode = Get-ConfiguredInstallMode -Context $context -BootstrapInstall:$BootstrapInstall

if ($Check) {
    Write-Host ''
    Write-Host '>>> Running Install Check'
    & (Join-Path $PSScriptRoot 'Install-GalPlugins.ps1') -RepoRoot $context.RepoRoot -SelectedRuntimes @($context.SelectedRuntimes) -PrimaryRuntime $context.PrimaryRuntime -Check

    if ($null -eq $previousBootstrapEnv) {
        Remove-Item Env:GAL_BOOTSTRAP_INSTALL -ErrorAction SilentlyContinue
    }
    else {
        $env:GAL_BOOTSTRAP_INSTALL = $previousBootstrapEnv
    }

    Write-Host ''
    Write-Host 'Check complete. No changes made.'
    return
}

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
    [pscustomobject]@{ Name = 'Machine Surfaces'; Path = $null },
    [pscustomobject]@{ Name = 'MCP'; Path = $null },
    [pscustomobject]@{ Name = 'Install Orchestration'; Path = Join-Path $PSScriptRoot 'Install-GalPlugins.ps1' }
)

if ($Purge -and -not $Uninstall) {
    throw 'Purge is only supported together with -Uninstall.'
}

if ($ConfirmPurge -and -not $Purge) {
    throw 'ConfirmPurge is only supported together with -Purge.'
}

foreach ($step in $steps) {
    Write-Host ''
    Write-Host ('>>> Running {0}' -f $step.Name)

    if ($step.Name -eq 'Machine Surfaces') {
        $updateArgs = @('update', '--machine-only')
        if ($DryRun) { $updateArgs += '--dry-run' }
        if ($Uninstall) { $updateArgs += '--uninstall' }
        if ($Replace) { $updateArgs += '--replace' }
        if (-not $Uninstall) {
            $updateArgs += @('--selected-runtimes', (@($context.SelectedRuntimes) -join ','))
            $updateArgs += @('--primary-runtime', $context.PrimaryRuntime)
        }
        & $galExe @updateArgs
        continue
    }

    if ($step.Name -eq 'MCP') {
        if (-not $Uninstall) {
            & $galExe mcp update
        }
        else {
            Write-Host '  [SKIP] MCP config files are preserved during uninstall.'
        }
        continue
    }

    $stepArguments = @{}
    foreach ($entry in $sharedArguments.GetEnumerator()) {
        $stepArguments[$entry.Key] = $entry.Value
    }

    if ($step.Name -eq 'Install Orchestration' -and $BootstrapInstall) {
        $stepArguments['BootstrapInstall'] = $true
    }

    if ($step.Name -eq 'Install Orchestration' -and $Purge) {
        $stepArguments['Purge'] = $true
    }

    if ($step.Name -eq 'Install Orchestration' -and $ConfirmPurge) {
        $stepArguments['ConfirmPurge'] = $true
    }

    & $step.Path @stepArguments
}

if ($null -eq $previousBootstrapEnv) {
    Remove-Item Env:GAL_BOOTSTRAP_INSTALL -ErrorAction SilentlyContinue
}
else {
    $env:GAL_BOOTSTRAP_INSTALL = $previousBootstrapEnv
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
    if ($installMode -eq 'source') {
        Write-Host 'Note: If SKILL.template.md or SKILL.local.md changes, rerun gal update --machine-only or Setup-Machine.ps1.'
    }
    else {
        Write-Host 'Note: Source-only skills and commands updates were skipped because install mode uses provider-native projections.'
        if ($BootstrapInstall) {
            Write-Host 'Note: Bootstrap install seeded install mode for first launch; switch to source mode later only if you set galRoot and devMode explicitly.'
        }
    }
}
