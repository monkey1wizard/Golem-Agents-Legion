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

function Invoke-UpdateSkills {
    $context = $script:SetupContext
    Ensure-SetupDirectories @(
        $context.CopilotRoot,
        $context.AgentsTarget,
        $context.SkillsTarget,
        $context.SharedAgentsRoot,
        $context.SharedSkillsTarget,
        $context.GeminiRoot,
        $context.GeminiSkillsTarget,
        $context.CodexRoot,
        $context.CodexSkillsTarget,
        $context.ClaudeRoot,
        $context.ClaudeSkillsTarget
    )

    $agentSourceDir = Join-Path $context.RepoRoot 'agent'
    $agentFiles = Get-ChildItem $agentSourceDir -Filter '*.agent.md' -File
    Write-Host ''
    Write-Host ("=== Agents ({0} files) ===" -f $agentFiles.Count)
    foreach ($agentFile in $agentFiles) {
        $linkPath = Join-Path $context.AgentsTarget $agentFile.Name
        if ($script:SetupOptions.Uninstall -or -not $context.InstallCopilot) {
            Remove-SafeLink $linkPath
        }
        else {
            New-SafeSymlink $linkPath $agentFile.FullName 'File' | Out-Null
        }
    }

    $skillSourceDir = Join-Path $context.RepoRoot 'skills'
    $skillDirs = Get-ChildItem $skillSourceDir -Directory
    Write-Host ''
    Write-Host ("=== Skills ({0} directories) ===" -f $skillDirs.Count)
    foreach ($skillDir in $skillDirs) {
        $linkPath = Join-Path $context.SkillsTarget $skillDir.Name
        if ($script:SetupOptions.Uninstall -or -not $context.InstallCopilot) {
            Remove-SafeLink $linkPath
        }
        else {
            New-SafeSymlink $linkPath $skillDir.FullName 'Directory' | Out-Null
        }
    }

    Write-Host ''
    Write-Host '=== Migration: .gemini/skills cleanup ==='
    $allGalSkillNames = @($skillDirs | ForEach-Object { $_.Name }) + $context.ActiveCommandSkillNames + @('gal.bak')
    foreach ($name in $allGalSkillNames) {
        $linkPath = Join-Path $context.GeminiSkillsTarget $name
        if (-not (Test-Path $linkPath)) { continue }
        $item = Get-Item $linkPath -Force -ErrorAction SilentlyContinue
        if ($null -eq $item) { continue }
        $isReparsePoint = ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0
        $isGalLink = $isReparsePoint -and (($item.Target -join ';') -like "*$($context.RepoRoot)*")
        if ($isGalLink -or $name -eq 'gal.bak') {
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would remove: $linkPath"
            }
            else {
                Remove-Item $linkPath -Recurse -Force
                Write-Host "  [REMOVED] $linkPath"
            }
        }
    }

    Write-Host ''
    Write-Host ("=== Shared Skills - Gemini + Codex ({0} reusable directories via .agents) ===" -f $skillDirs.Count)
    foreach ($skillDir in $skillDirs) {
        $linkPath = Join-Path $context.SharedSkillsTarget $skillDir.Name
        if ($script:SetupOptions.Uninstall -or -not $context.InstallSharedSkills) {
            Remove-SafeLink $linkPath
        }
        else {
            New-SafeSymlink $linkPath $skillDir.FullName 'Directory' | Out-Null
        }
    }

    Write-Host ''
    Write-Host ("=== Claude Skills ({0} reusable directories) ===" -f $skillDirs.Count)
    foreach ($skillDir in $skillDirs) {
        $linkPath = Join-Path $context.ClaudeSkillsTarget $skillDir.Name
        if ($script:SetupOptions.Uninstall -or -not $context.InstallClaude) {
            Remove-SafeLink $linkPath
        }
        else {
            New-SafeSymlink $linkPath $skillDir.FullName 'Directory' | Out-Null
        }
    }

    Write-Host ''
    Write-Host '=== GAL_ROOT symlinks ==='
    if ($script:SetupOptions.Uninstall -or -not $context.InstallCopilot) {
        Remove-SafeLink $context.GalRootCopilot
    }
    else {
        New-SafeSymlink $context.GalRootCopilot $context.RepoRoot 'Directory' | Out-Null
    }

    if ($script:SetupOptions.Uninstall -or -not $context.InstallGemini) {
        Remove-SafeLink $context.GalRootGemini
    }
    else {
        New-SafeSymlink $context.GalRootGemini $context.RepoRoot 'Directory' | Out-Null
    }
}

Initialize-SetupSession -EntryScriptPath $MyInvocation.MyCommand.Path -Uninstall:$Uninstall -Replace:$Replace -DryRun:$DryRun -Reconfigure:$Reconfigure -SelectedRuntimes $SelectedRuntimes -PrimaryRuntime $PrimaryRuntime | Out-Null
Invoke-UpdateSkills