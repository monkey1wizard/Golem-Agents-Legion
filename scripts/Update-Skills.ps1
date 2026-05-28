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

function Get-AgentFrontmatterValue([string]$RawContent, [string]$FieldName) {
    if ($RawContent -match '(?ms)^---\r?\n(?<frontmatter>.*?)\r?\n---\r?\n?') {
        $frontmatter = $matches['frontmatter']
        if ($frontmatter -match ("(?m)^{0}:\s*\[(?<value>.*?)\]\s*$" -f [regex]::Escape($FieldName))) {
            return $matches['value'].Trim()
        }
        if ($frontmatter -match ('(?m)^{0}:\s*"(?<value>.*)"\s*$' -f [regex]::Escape($FieldName))) {
            return $matches['value'].Trim()
        }
        if ($frontmatter -match ("(?m)^{0}:\s*(?<value>.+?)\s*$" -f [regex]::Escape($FieldName))) {
            return $matches['value'].Trim().Trim('"')
        }
    }

    return $null
}

function Get-AgentTools([string]$RawContent) {
    $value = Get-AgentFrontmatterValue -RawContent $RawContent -FieldName 'tools'
    if ([string]::IsNullOrWhiteSpace($value)) {
        return @()
    }

    return @($value -split ',' | ForEach-Object {
        $_.Trim().Trim([char[]]@([char]39, [char]34))
    } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
}

function Get-AgentMarkdownBody([string]$RawContent) {
    if ($RawContent -match '(?ms)^---\r?\n.*?\r?\n---\r?\n?(?<body>.*)$') {
        return $matches['body']
    }

    return $RawContent
}

function Get-OpenCodePermissionLines([string[]]$Tools) {
    $permissions = [ordered]@{}
    foreach ($tool in $Tools) {
        switch ($tool) {
            'read' {
                $permissions['read'] = 'allow'
                $permissions['list'] = 'allow'
            }
            'search' {
                $permissions['read'] = 'allow'
                $permissions['list'] = 'allow'
                $permissions['grep'] = 'allow'
                $permissions['glob'] = 'allow'
            }
            'edit' {
                $permissions['edit'] = 'allow'
            }
            'execute' {
                $permissions['bash'] = 'allow'
            }
        }
    }

    if ($permissions.Count -eq 0) {
        $permissions['read'] = 'allow'
        $permissions['list'] = 'allow'
    }

    return @($permissions.GetEnumerator() | ForEach-Object { '  {0}: {1}' -f $_.Key, $_.Value })
}

function New-OpenCodeAgentFileContent([string]$AgentPath) {
    $rawContent = Get-Content $AgentPath -Raw -Encoding UTF8
    $description = Get-AgentFrontmatterValue -RawContent $rawContent -FieldName 'description'
    if ([string]::IsNullOrWhiteSpace($description)) {
        $description = 'GAL golem agent'
    }

    $color = Get-AgentFrontmatterValue -RawContent $rawContent -FieldName 'color'
    $body = (Get-AgentMarkdownBody $rawContent).Trim()
    $permissionLines = Get-OpenCodePermissionLines -Tools (Get-AgentTools $rawContent)

    $content = [System.Collections.Generic.List[string]]::new()
    $content.Add($script:SetupContext.GalManagedFileHeader)
    $content.Add('---')
    $content.Add('description: |')
    foreach ($line in (($description -replace "`r`n", "`n" -replace "`r", "`n") -split "`n")) {
        $content.Add('  ' + $line)
    }
    $content.Add('mode: subagent')
    if (-not [string]::IsNullOrWhiteSpace($color)) {
        $content.Add('color: ' + $color)
    }
    $content.Add('permission:')
    foreach ($line in $permissionLines) {
        $content.Add($line)
    }
    $content.Add('---')
    $content.Add('')
    $content.Add($body)
    $content.Add('')

    return ($content -join "`n")
}

function Invoke-UpdateSkills {
    $context = $script:SetupContext
    $installMode = Test-InstallModeFromContext -Context $context
    Ensure-SetupDirectories @(
        $context.CopilotRoot,
        $context.AgentsTarget,
        $context.SkillsTarget,
        $context.SharedAgentsRoot,
        $context.SharedSkillsTarget,
        $context.GeminiRoot,
        $context.GeminiSkillsTarget,
        $context.AntigravityRoot,
        $context.AntigravitySkillsTarget,
        $context.CodexRoot,
        $context.CodexSkillsTarget,
        $context.OpenCodeRoot,
        $context.OpenCodeAgentsTarget,
        $context.GalGeneratedProvidersRoot
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

    Write-Host ''
    Write-Host ("=== OpenCode Agents ({0} generated subagents) ===" -f $agentFiles.Count)
    foreach ($agentFile in $agentFiles) {
        $agentName = [System.IO.Path]::GetFileNameWithoutExtension([System.IO.Path]::GetFileNameWithoutExtension($agentFile.Name))
        $targetPath = Join-Path $context.OpenCodeAgentsTarget ("{0}.md" -f $agentName)
        if ($script:SetupOptions.Uninstall -or -not $context.InstallOpenCode) {
            if (-not (Test-Path $targetPath)) { continue }
            if (-not (Test-GalManagedFile $targetPath)) {
                Write-Host "  [SKIP] User-owned OpenCode agent preserved: $targetPath"
                continue
            }
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would remove: $targetPath"
            }
            else {
                Remove-Item $targetPath -Force
                Write-Host "  [REMOVED] $targetPath"
            }
        }
        else {
            $agentContent = New-OpenCodeAgentFileContent -AgentPath $agentFile.FullName
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would write: $targetPath"
            }
            else {
                [System.IO.File]::WriteAllText($targetPath, $agentContent, $context.Utf8NoBom)
                Write-Host "  [OK] $targetPath"
            }
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
    Write-Host '=== AGY Plugin (skills + command skills + agents) ==='
    if ($installMode) {
        Write-Host '  [SKIP] Install mode delegates AGY plugin lifecycle to Install-GalPlugins.ps1.'
    }
    elseif ($script:SetupOptions.Uninstall -or -not $context.InstallAntigravity) {
        # Legacy cleanup: remove old AGY skill symlinks under antigravity-cli/skills/
        foreach ($skillDir in $skillDirs) {
            $legacyLink = Join-Path $context.AntigravitySkillsTarget $skillDir.Name
            Remove-SafeLink $legacyLink
        }
        # Also clean up legacy command skill symlinks
        foreach ($cmdName in $context.ActiveCommandSkillNames) {
            $legacyLink = Join-Path $context.AntigravitySkillsTarget $cmdName
            Remove-SafeLink $legacyLink
        }
        # Remove the installed plugin directory
        if (Test-Path $context.AgyPluginInstallTarget) {
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would remove: $($context.AgyPluginInstallTarget)"
            }
            else {
                Remove-Item $context.AgyPluginInstallTarget -Recurse -Force
                Write-Host "  [REMOVED] $($context.AgyPluginInstallTarget)"
            }
        }
    }
    else {
        # Legacy cleanup: remove old AGY skill symlinks under antigravity-cli/skills/
        foreach ($skillDir in $skillDirs) {
            $legacyLink = Join-Path $context.AntigravitySkillsTarget $skillDir.Name
            Remove-SafeLink $legacyLink
        }
        foreach ($cmdName in $context.ActiveCommandSkillNames) {
            $legacyLink = Join-Path $context.AntigravitySkillsTarget $cmdName
            Remove-SafeLink $legacyLink
        }

        # Build and install the AGY plugin (includes reusable skills, command skills, agents)
        $buildScript = Join-Path $PSScriptRoot 'Build-AgyPlugin.ps1'
        if (Test-Path $buildScript) {
            if ($script:SetupOptions.DryRun) {
                Write-Host '  [DRY RUN] Would run: Build-AgyPlugin.ps1 -Force -Install'
            }
            else {
                & $buildScript -Force -Install
                Write-Host '  [OK] AGY plugin built and installed'
            }
        }
        else {
            Write-Host "  [WARN] Build-AgyPlugin.ps1 not found at: $buildScript"
        }
    }

    Write-Host ''
    Write-Host '=== Migration: repo .agents skills cleanup ==='
    foreach ($skillDir in $skillDirs) {
        $linkPath = Join-Path $context.WorkspaceSkillsTarget $skillDir.Name
        Remove-SafeLink $linkPath
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

    Write-Host ("=== Shared Skills - Codex + OpenCode ({0} reusable directories via ~/.agents) ===" -f $skillDirs.Count)
    foreach ($skillDir in $skillDirs) {
        $linkPath = Join-Path $context.SharedSkillsTarget $skillDir.Name
        if ($script:SetupOptions.Uninstall -or (-not $context.InstallCodex -and -not $context.InstallOpenCode)) {
            Remove-SafeLink $linkPath
        }
        else {
            New-SafeSymlink $linkPath $skillDir.FullName 'Directory' | Out-Null
        }
    }

    Write-Host ''
    Write-Host ("=== Claude legacy skills cleanup ({0} reusable directories) ===" -f $skillDirs.Count)
    $claudeSkillsTarget = Join-Path $env:USERPROFILE '.claude\skills'
    foreach ($skillDir in $skillDirs) {
        $linkPath = Join-Path $claudeSkillsTarget $skillDir.Name
        if (-not (Test-GalRepoLink $linkPath)) { continue }
        if ($script:SetupOptions.DryRun) {
            Write-Host "  [DRY RUN] Would remove: $linkPath"
        }
        else {
            Remove-Item $linkPath -Recurse -Force
            Write-Host "  [REMOVED] $linkPath"
        }
    }

    Write-Host ''
    Write-Host '=== GAL_ROOT symlinks ==='
    $shouldKeepGalSourceLink = -not $script:SetupOptions.Uninstall -and ($context.InstallCopilot -or $context.InstallGemini -or $context.InstallAntigravity)
    if ($shouldKeepGalSourceLink) {
        New-SafeSymlink $context.GalSourceRoot $context.RepoRoot 'Directory' | Out-Null
    }
    else {
        Remove-SafeLink $context.GalSourceRoot
    }

    if ($script:SetupOptions.Uninstall -or -not $context.InstallCopilot) {
        Remove-SafeLink $context.GalRootCopilot
    }
    else {
        New-SafeSymlink $context.GalRootCopilot $context.GalSourceRoot 'Directory' | Out-Null
    }

    if ($script:SetupOptions.Uninstall -or -not $context.InstallGemini) {
        Remove-SafeLink $context.GalRootGemini
    }
    else {
        New-SafeSymlink $context.GalRootGemini $context.GalSourceRoot 'Directory' | Out-Null
    }

    if ($script:SetupOptions.Uninstall -or -not $context.InstallAntigravity) {
        Remove-SafeLink $context.GalRootAntigravity
    }
    else {
        New-SafeSymlink $context.GalRootAntigravity $context.GalSourceRoot 'Directory' | Out-Null
    }
}

Initialize-SetupSession -EntryScriptPath $MyInvocation.MyCommand.Path -Uninstall:$Uninstall -Replace:$Replace -DryRun:$DryRun -Reconfigure:$Reconfigure -SelectedRuntimes $SelectedRuntimes -PrimaryRuntime $PrimaryRuntime | Out-Null
Invoke-UpdateSkills