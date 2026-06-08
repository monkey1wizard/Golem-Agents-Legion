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

function Get-BakedCommandSkillContent([string]$TemplatePath, [string]$LocalOverridePath) {
    $baked = (Get-Content $TemplatePath -Raw) -replace [regex]::Escape('{{GAL_ROOT}}'), $script:SetupContext.RepoRoot

    if (-not (Test-Path $LocalOverridePath)) {
        return $baked
    }

    $localOverride = Get-Content $LocalOverridePath -Raw
    if ([string]::IsNullOrWhiteSpace($localOverride)) {
        return $baked
    }

    return @(
        $baked.TrimEnd("`r", "`n")
        ''
        '<!-- GAL LOCAL OVERRIDE START -->'
        '<!-- Source: SKILL.local.md (gitignored machine-local overlay) -->'
        $localOverride.Trim("`r", "`n")
        '<!-- GAL LOCAL OVERRIDE END -->'
        ''
    ) -join "`n"
}

function Get-SkillFrontmatterDescription([string]$RawContent) {
    if ($RawContent -match '(?ms)^---\r?\n(?<frontmatter>.*?)\r?\n---\r?\n?') {
        $frontmatter = $matches['frontmatter']
        if ($frontmatter -match '(?m)^description:\s*"(?<description>.*)"\s*$') {
            return $matches['description'].Trim()
        }
        if ($frontmatter -match '(?m)^description:\s*(?<description>.+?)\s*$') {
            return $matches['description'].Trim().Trim('"')
        }
    }

    return $null
}

function Get-SkillMarkdownBody([string]$RawContent) {
    if ($RawContent -match '(?ms)^---\r?\n.*?\r?\n---\r?\n?(?<body>.*)$') {
        return $matches['body']
    }

    return $RawContent
}

function Convert-ToTomlMultilineLiteralString([string]$Text) {
    $normalized = $Text -replace "`r`n", "`n" -replace "`r", "`n"
    return "'''" + "`n" + $normalized.TrimEnd() + "`n" + "'''"
}

function New-GeminiCommandFileContent([string]$SkillPath) {
    $rawContent = Get-Content $SkillPath -Raw -Encoding UTF8
    $description = Get-SkillFrontmatterDescription $rawContent
    if ([string]::IsNullOrWhiteSpace($description)) {
        $description = 'GAL command'
    }

    $body = (Get-SkillMarkdownBody $rawContent).Trim()
    $prompt = @(
        'User command arguments, if any: {{args}}'
        ''
        $body
    ) -join "`n"

    $escapedDescription = $description.Replace('\\', '\\\\').Replace('"', '\\"')

    return @(
        $script:SetupContext.GalManagedFileHeader
        ('description = "{0}"' -f $escapedDescription)
        ("prompt = {0}" -f (Convert-ToTomlMultilineLiteralString $prompt))
        ''
    ) -join "`n"
}

function New-ClaudeCommandFileContent([string]$SkillPath, [string]$CommandName) {
    $rawContent = Get-Content $SkillPath -Raw -Encoding UTF8
    $description = Get-SkillFrontmatterDescription $rawContent
    if ([string]::IsNullOrWhiteSpace($description)) {
        $description = 'GAL command'
    }

    $body = (Get-SkillMarkdownBody $rawContent).Trim()
    $indentedDescription = ($description -replace "`r`n", "`n" -replace "`r", "`n") -split "`n" | ForEach-Object { '  ' + $_ }

    return @(
        $script:SetupContext.GalManagedFileHeader
        '---'
        'description: |'
        ($indentedDescription -join "`n")
        '---'
        ''
        ('# {0}' -f $CommandName)
        ''
        'User command arguments, if any: {{args}}'
        ''
        $body
        ''
    ) -join "`n"
}

function New-OpenCodeCommandFileContent([string]$SkillPath, [string]$CommandName) {
    $rawContent = Get-Content $SkillPath -Raw -Encoding UTF8
    $description = Get-SkillFrontmatterDescription $rawContent
    if ([string]::IsNullOrWhiteSpace($description)) {
        $description = 'GAL command'
    }

    $body = if ($CommandName -eq 'git-commit-msg') {
        @(
            'Run the repo helper below and return its output exactly. The helper decides whether the output is header-only or includes a body, so do not invent bullets or rewrite the summary. Do not add explanations, markdown fences, reasoning tags, JSON, or any extra prose. If the helper reports No changes staged for commit. or Not a git repository., return that text exactly. Apply extra instructions if provided: $ARGUMENTS'
            ''
            '!`pwsh -NoProfile -File ./scripts/Get-StagedCommitMessage.ps1`'
        ) -join "`n"
    }
    else {
        (Get-SkillMarkdownBody $rawContent).Trim()
    }
    $indentedDescription = ($description -replace "`r`n", "`n" -replace "`r", "`n") -split "`n" | ForEach-Object { '  ' + $_ }

    return @(
        $script:SetupContext.GalManagedFileHeader
        '---'
        'description: |'
        ($indentedDescription -join "`n")
        '---'
        ''
        'User command arguments, if any: $ARGUMENTS'
        ''
        $body
        ''
    ) -join "`n"
}

function Resolve-CommandSkillContentPath([pscustomobject]$CommandSkill) {
    $bakedSkillPath = Join-Path $CommandSkill.Source 'SKILL.md'
    if (Test-Path $bakedSkillPath) {
        return $bakedSkillPath
    }

    if (Test-Path $CommandSkill.Template) {
        return $CommandSkill.Template
    }

    return $bakedSkillPath
}

function Invoke-UpdateCommands {
    $context = $script:SetupContext
    Ensure-SetupDirectories @(
        $context.SkillsTarget,
        $context.CodexSkillsTarget,
        $context.GeminiCommandsTarget,
        $context.AntigravitySkillsTarget,
        $context.OpenCodeCommandsTarget,
        $context.SharedSkillsTarget
    )

    Write-Host ''
    Write-Host '=== Generated GAL command skills ==='
    if ($script:SetupOptions.Uninstall -or -not $context.NeedsBakedCommandSkills) {
        foreach ($commandSkill in $context.CommandSkillDirs) {
            $bakedSkill = Join-Path $commandSkill.Source 'SKILL.md'
            if (Test-Path $bakedSkill) {
                if ($script:SetupOptions.DryRun) {
                    Write-Host "  [DRY RUN] Would remove baked: $bakedSkill"
                }
                else {
                    Remove-Item $bakedSkill -Force
                    Write-Host "  [REMOVED] $bakedSkill"
                }
            }
        }
    }
    else {
        foreach ($commandSkill in $context.CommandSkillDirs) {
            if (-not (Test-Path $commandSkill.Template)) {
                Write-Host "  [WARN] Template not found: $($commandSkill.Template)"
                continue
            }

            $localOverridePath = Join-Path $commandSkill.Source 'SKILL.local.md'
            $baked = Get-BakedCommandSkillContent -TemplatePath $commandSkill.Template -LocalOverridePath $localOverridePath
            $bakedSkill = Join-Path $commandSkill.Source 'SKILL.md'
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would write baked: $bakedSkill"
            }
            else {
                [System.IO.File]::WriteAllText($bakedSkill, $baked, $context.Utf8NoBom)
                Write-Host "  [OK] $bakedSkill"
            }
        }
    }

    Write-Host ''
    Write-Host '=== GAL command skill symlinks (legacy Copilot cleanup + Codex) ==='
    foreach ($commandSkill in $context.CommandSkillDirs) {
        Remove-SafeLink $commandSkill.CopilotTarget

        if ($script:SetupOptions.Uninstall -or -not $context.InstallCodex) {
            Remove-SafeLink $commandSkill.CodexTarget
        }
        else {
            New-SafeSymlink $commandSkill.CodexTarget $commandSkill.Source 'Directory' | Out-Null
        }
    }

    Write-Host ''
    Write-Host '=== AGY command skill legacy cleanup ==='
    # AGY command skills are now rendered by Build-CorePlugin.ps1 (called from Update-Skills).
    # This section only cleans up legacy symlinks that predate the plugin model.
    foreach ($commandSkill in $context.CommandSkillDirs) {
        $antigravityTarget = Join-Path $context.AntigravitySkillsTarget $commandSkill.Name
        Remove-SafeLink $antigravityTarget
    }

    Write-Host ''
    Write-Host '=== Migration: repo .agents command cleanup ==='
    foreach ($commandSkill in $context.CommandSkillDirs) {
        $workspaceTarget = Join-Path $context.WorkspaceSkillsTarget $commandSkill.Name
        Remove-SafeLink $workspaceTarget
    }

    Write-Host ''
    Write-Host '=== Migration: ~/.agents command cleanup ==='
    foreach ($commandSkill in $context.CommandSkillDirs) {
        $sharedCommandPath = Join-Path $context.SharedSkillsTarget $commandSkill.Name
        if (-not (Test-Path $sharedCommandPath)) { continue }
        if (-not (Test-GalRepoLink $sharedCommandPath)) {
            Write-Host "  [SKIP] User-owned shared skill preserved: $sharedCommandPath"
            continue
        }
        if ($script:SetupOptions.DryRun) {
            Write-Host "  [DRY RUN] Would remove: $sharedCommandPath"
        }
        else {
            Remove-Item $sharedCommandPath -Recurse -Force
            Write-Host "  [REMOVED] $sharedCommandPath"
        }
    }

    Write-Host ''
    Write-Host '=== Gemini custom commands ==='
    foreach ($commandSkill in $context.CommandSkillDirs) {
        $commandFile = Join-Path $context.GeminiCommandsTarget ("{0}.toml" -f $commandSkill.Name)
        if ($script:SetupOptions.Uninstall -or -not $context.InstallGemini) {
            if (-not (Test-Path $commandFile)) { continue }
            if (-not (Test-GalManagedFile $commandFile)) {
                Write-Host "  [SKIP] User-owned Gemini command preserved: $commandFile"
                continue
            }
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would remove: $commandFile"
            }
            else {
                Remove-Item $commandFile -Force
                Write-Host "  [REMOVED] $commandFile"
            }
        }
        else {
            $commandContent = New-GeminiCommandFileContent (Resolve-CommandSkillContentPath -CommandSkill $commandSkill)
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would write: $commandFile"
            }
            else {
                [System.IO.File]::WriteAllText($commandFile, $commandContent, $context.Utf8NoBom)
                Write-Host "  [OK] $commandFile"
            }
        }
    }

    Write-Host ''
    Write-Host '=== Claude legacy command cleanup ==='
    $claudeCommandsTarget = Join-Path $env:USERPROFILE '.claude\commands'
    foreach ($commandSkill in $context.CommandSkillDirs) {
        $commandFile = Join-Path $claudeCommandsTarget ("{0}.md" -f $commandSkill.Name)
        if (-not (Test-Path $commandFile)) { continue }
        if (-not (Test-GalManagedFile $commandFile)) {
            Write-Host "  [SKIP] User-owned Claude command preserved: $commandFile"
            continue
        }
        if ($script:SetupOptions.DryRun) {
            Write-Host "  [DRY RUN] Would remove: $commandFile"
        }
        else {
            Remove-Item $commandFile -Force
            Write-Host "  [REMOVED] $commandFile"
        }
    }

    Write-Host ''
    Write-Host '=== OpenCode custom commands ==='
    foreach ($commandSkill in $context.CommandSkillDirs) {
        $commandFile = Join-Path $context.OpenCodeCommandsTarget ("{0}.md" -f $commandSkill.Name)
        if ($script:SetupOptions.Uninstall -or -not $context.InstallOpenCode) {
            if (-not (Test-Path $commandFile)) { continue }
            if (-not (Test-GalManagedFile $commandFile)) {
                Write-Host "  [SKIP] User-owned OpenCode command preserved: $commandFile"
                continue
            }
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would remove: $commandFile"
            }
            else {
                Remove-Item $commandFile -Force
                Write-Host "  [REMOVED] $commandFile"
            }
        }
        else {
            $commandContent = New-OpenCodeCommandFileContent -SkillPath (Resolve-CommandSkillContentPath -CommandSkill $commandSkill) -CommandName $commandSkill.Name
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would write: $commandFile"
            }
            else {
                [System.IO.File]::WriteAllText($commandFile, $commandContent, $context.Utf8NoBom)
                Write-Host "  [OK] $commandFile"
            }
        }
    }

    Write-Host ''
    Write-Host '=== Migration: obsolete command cleanup ==='
    foreach ($skillsDir in @($context.SkillsTarget, $context.GeminiSkillsTarget, $context.AntigravitySkillsTarget, $context.SharedSkillsTarget, $context.CodexSkillsTarget)) {
        $obsoleteCommandLinks = Get-ChildItem $skillsDir -Directory -ErrorAction SilentlyContinue | Where-Object {
            $_.Name -notin $context.ActiveCommandSkillNames -and (Test-GalCommandLink $_.FullName)
        }

        foreach ($dir in $obsoleteCommandLinks) {
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would remove obsolete command link: $($dir.FullName)"
            }
            else {
                Remove-Item $dir.FullName -Recurse -Force
                Write-Host "  [REMOVED] Obsolete command link: $($dir.FullName)"
            }
        }
    }

    foreach ($commandFile in (Get-ChildItem $context.GeminiCommandsTarget -Filter '*.toml' -File -ErrorAction SilentlyContinue | Where-Object {
        $_.BaseName -notin $context.ActiveCommandSkillNames -and (Test-GalManagedFile $_.FullName)
    })) {
        if ($script:SetupOptions.DryRun) {
            Write-Host "  [DRY RUN] Would remove obsolete Gemini command: $($commandFile.FullName)"
        }
        else {
            Remove-Item $commandFile.FullName -Force
            Write-Host "  [REMOVED] Obsolete Gemini command: $($commandFile.FullName)"
        }
    }

    foreach ($commandFile in (Get-ChildItem $claudeCommandsTarget -Filter '*.md' -File -ErrorAction SilentlyContinue | Where-Object {
        $_.BaseName -notin $context.ActiveCommandSkillNames -and (Test-GalManagedFile $_.FullName)
    })) {
        if ($script:SetupOptions.DryRun) {
            Write-Host "  [DRY RUN] Would remove obsolete Claude command: $($commandFile.FullName)"
        }
        else {
            Remove-Item $commandFile.FullName -Force
            Write-Host "  [REMOVED] Obsolete Claude command: $($commandFile.FullName)"
        }
    }

    foreach ($commandFile in (Get-ChildItem $context.OpenCodeCommandsTarget -Filter '*.md' -File -ErrorAction SilentlyContinue | Where-Object {
        $_.BaseName -notin $context.ActiveCommandSkillNames -and (Test-GalManagedFile $_.FullName)
    })) {
        if ($script:SetupOptions.DryRun) {
            Write-Host "  [DRY RUN] Would remove obsolete OpenCode command: $($commandFile.FullName)"
        }
        else {
            Remove-Item $commandFile.FullName -Force
            Write-Host "  [REMOVED] Obsolete OpenCode command: $($commandFile.FullName)"
        }
    }
}

Initialize-SetupSession -EntryScriptPath $MyInvocation.MyCommand.Path -Uninstall:$Uninstall -Replace:$Replace -DryRun:$DryRun -Reconfigure:$Reconfigure -SelectedRuntimes $SelectedRuntimes -PrimaryRuntime $PrimaryRuntime | Out-Null
Invoke-UpdateCommands