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

function Invoke-UpdatePersonalization {
    $context = $script:SetupContext
    $skillDirs = Get-ChildItem (Join-Path $context.RepoRoot 'skills') -Directory
    $installMode = Test-InstallModeFromContext -Context $context

    Ensure-SetupDirectories @($context.GalStateRoot, $context.GeminiRoot, $context.AntigravityRoot)

    Write-Host ''
    Write-Host '=== Gemini gal-context.md ==='
    if ($script:SetupOptions.Uninstall -or -not $context.InstallGemini) {
        if (Test-Path $context.GeminiContextFile) {
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would remove: $($context.GeminiContextFile)"
            }
            else {
                Remove-Item $context.GeminiContextFile -Force
                Write-Host "  [REMOVED] $($context.GeminiContextFile)"
            }
        }
    }
    else {
        $skillImports = $skillDirs | Sort-Object Name | ForEach-Object {
            '@' + (Join-Path $_.FullName 'SKILL.md')
        }
        $contextContent = ($skillImports -join "`n") + "`n"

        if ($script:SetupOptions.DryRun) {
            Write-Host "  [DRY RUN] Would write: $($context.GeminiContextFile) ($($skillDirs.Count) skill imports)"
        }
        else {
            [System.IO.File]::WriteAllText($context.GeminiContextFile, $contextContent, $context.Utf8NoBom)
            Write-Host "  [OK] $($context.GeminiContextFile) ($($skillDirs.Count) skill imports)"
        }
    }

    Write-Host ''
    Write-Host '=== Gemini settings.json bridge ==='
    if ($script:SetupOptions.Uninstall) {
        Write-Host '  [SKIP] settings.json not modified during uninstall (user-owned file)'
    }
    elseif (-not $context.InstallGemini) {
        Write-Host '  [SKIP] Gemini runtime not selected; settings.json bridge not updated'
    }
    elseif ($script:SetupOptions.DryRun) {
        Write-Host "  [DRY RUN] Would merge AGENTS.md into context.fileName in: $($context.GeminiSettingsFile)"
    }
    else {
        if (Test-Path $context.GeminiSettingsFile) {
            $rawJson = Get-Content $context.GeminiSettingsFile -Raw -Encoding UTF8
            try { $settings = $rawJson | ConvertFrom-Json }
            catch { Write-Host "  [WARN] Could not parse $($context.GeminiSettingsFile) as JSON — skipping bridge" -ForegroundColor Yellow; $settings = $null }
        }
        else {
            $settings = [pscustomobject]@{}
        }

        if ($null -ne $settings) {
            if (-not (Get-Member -InputObject $settings -Name 'context' -MemberType NoteProperty)) {
                Add-Member -InputObject $settings -MemberType NoteProperty -Name 'context' -Value ([pscustomobject]@{})
            }
            if (-not (Get-Member -InputObject $settings.context -Name 'fileName' -MemberType NoteProperty)) {
                Add-Member -InputObject $settings.context -MemberType NoteProperty -Name 'fileName' -Value @('AGENTS.md', 'GEMINI.md')
            }
            else {
                $current = @($settings.context.fileName)
                foreach ($required in @('AGENTS.md', 'GEMINI.md')) {
                    if ($current -notcontains $required) { $current += $required }
                }
                $settings.context.fileName = $current
            }

            [System.IO.File]::WriteAllText($context.GeminiSettingsFile, ($settings | ConvertTo-Json -Depth 10), $context.Utf8NoBom)
            Write-Host "  [OK] $($context.GeminiSettingsFile) (context.fileName includes AGENTS.md and GEMINI.md)"
        }
    }

    Write-Host ''
    Write-Host '=== AGY Plugin (rules/gal.md) ==='
    if ($installMode) {
        Write-Host '  [SKIP] Install mode delegates AGY plugin lifecycle to Install-GalPlugins.ps1.'
    }
    elseif ($script:SetupOptions.Uninstall -or -not $context.InstallAntigravity) {
        # Remove the installed plugin directory on uninstall or when AGY is not selected
        if (Test-Path $context.AgyPluginInstallTarget) {
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would remove: $($context.AgyPluginInstallTarget)"
            }
            else {
                Remove-Item $context.AgyPluginInstallTarget -Recurse -Force
                Write-Host "  [REMOVED] $($context.AgyPluginInstallTarget)"
            }
        }
        else {
            Write-Host '  [SKIP] No AGY plugin installation to remove'
        }
    }
    else {
        # T-003: Resolve mode and effective source root
        $installModeValue = Get-ConfiguredInstallModeFromContext -Context $script:SetupContext
        $effectiveRepoRoot = if ($installModeValue -eq 'source') {
            $config = if (Test-Path $script:SetupContext.GalConfigFile) { Read-JsonOrderedMap $script:SetupContext.GalConfigFile } else { $null }
            $galRoot = if ($config -and $config.Contains('galRoot')) { [string]$config['galRoot'] } else { (Resolve-Path (Join-Path $PSScriptRoot '..')).Path }
            if (-not [string]::IsNullOrWhiteSpace($galRoot)) { $galRoot } else { (Resolve-Path (Join-Path $PSScriptRoot '..')).Path }
        }
        else {
            (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
        }

        # Build and install the AGY plugin (includes rules/gal.md from instruction corpus)
        $buildScript = Join-Path $PSScriptRoot 'Build-CorePlugin.ps1'
        if (Test-Path $buildScript) {
            if ($script:SetupOptions.DryRun) {
                Write-Host '  [DRY RUN] Would run: Build-CorePlugin.ps1 -Force -Install'
            }
            else {
                & $buildScript -RepoRoot $effectiveRepoRoot -InstallMode $installModeValue -Force -Install
                Write-Host '  [OK] AGY plugin built and installed (rules/gal.md from instruction corpus)'
            }
        }
        else {
            Write-Host "  [WARN] Build-CorePlugin.ps1 not found at: $buildScript"
        }
    }

    Write-Host ''
    Write-Host '=== Migration: repo .agents cleanup ==='
    $workspaceRuleFile = Join-Path $context.WorkspaceRulesTarget 'gal.md'
    if (Test-Path $workspaceRuleFile) {
        if (-not (Test-GalManagedFile $workspaceRuleFile)) {
            Write-Host "  [SKIP] User-owned repo rule preserved: $workspaceRuleFile"
        }
        elseif ($script:SetupOptions.DryRun) {
            Write-Host "  [DRY RUN] Would remove: $workspaceRuleFile"
        }
        else {
            Remove-Item $workspaceRuleFile -Force
            Write-Host "  [REMOVED] $workspaceRuleFile"
        }
    }
    foreach ($dir in @($context.WorkspaceSkillsTarget, $context.WorkspaceRulesTarget, $context.WorkspaceAgentsRoot)) {
        if (-not (Test-Path $dir)) { continue }
        $hasChildren = @(Get-ChildItem $dir -Force -ErrorAction SilentlyContinue).Count -gt 0
        if ($hasChildren) {
            Write-Host "  [SKIP] Non-empty repo path preserved: $dir"
            continue
        }
        if ($script:SetupOptions.DryRun) {
            Write-Host "  [DRY RUN] Would remove empty directory: $dir"
        }
        else {
            Remove-Item $dir -Force
            Write-Host "  [REMOVED] Empty directory: $dir"
        }
    }

    Write-Host ''
    Write-Host '=== VS Code settings bridge ==='
    if ($script:SetupOptions.Uninstall) {
        Write-Host '  [SKIP] VS Code settings.json not modified during uninstall (user-owned file)'
    }
    elseif (-not $context.InstallCopilot) {
        Write-Host '  [SKIP] Copilot runtime not selected; VS Code settings bridge not updated'
    }
    elseif ($script:SetupOptions.DryRun) {
        Write-Host "  [DRY RUN] Would set chat.agentSkillsLocations['~/.agents/skills']=false in: $($context.VscodeSettingsFile)"
    }
    else {
        if (Test-Path $context.VscodeSettingsFile) {
            $rawJson = Get-Content $context.VscodeSettingsFile -Raw -Encoding UTF8
            try { $settings = $rawJson | ConvertFrom-Json }
            catch { Write-Host "  [WARN] Could not parse $($context.VscodeSettingsFile) as JSON — add chat.agentSkillsLocations manually" -ForegroundColor Yellow; $settings = $null }
        }
        else {
            $settingsDir = Split-Path $context.VscodeSettingsFile -Parent
            if (-not (Test-Path $settingsDir)) {
                New-Item -ItemType Directory -Path $settingsDir -Force | Out-Null
            }
            $settings = [pscustomobject]@{}
        }

        if ($null -ne $settings) {
            if (-not (Get-Member -InputObject $settings -Name 'chat.agentSkillsLocations' -MemberType NoteProperty)) {
                Add-Member -InputObject $settings -MemberType NoteProperty -Name 'chat.agentSkillsLocations' -Value ([pscustomobject]@{})
            }

            $skillLocations = $settings.'chat.agentSkillsLocations'
            if ($skillLocations -is [System.Collections.IDictionary]) {
                $skillLocations['~/.agents/skills'] = $false
            }
            elseif ($skillLocations -is [pscustomobject]) {
                if (Get-Member -InputObject $skillLocations -Name '~/.agents/skills' -MemberType NoteProperty) {
                    $skillLocations.'~/.agents/skills' = $false
                }
                else {
                    Add-Member -InputObject $skillLocations -MemberType NoteProperty -Name '~/.agents/skills' -Value $false
                }
            }

            [System.IO.File]::WriteAllText($context.VscodeSettingsFile, ($settings | ConvertTo-Json -Depth 10), $context.Utf8NoBom)
            Write-Host "  [OK] $($context.VscodeSettingsFile) (chat.agentSkillsLocations disables ~/.agents/skills for VS Code)"
        }
    }

    if ($script:SetupOptions.Uninstall -or $script:SetupOptions.DryRun) {
        return
    }

    Write-Host ''
    Write-Host '=== Personalization ==='

    $exampleEnv = Join-Path $context.RepoRoot 'config.example.env'
    $primaryLocalEnv = Join-Path $context.GalConfigRoot 'config.local.env'
    $legacyLocalEnv = Join-Path $context.RepoRoot 'config.local.env'
    $localEnv = $primaryLocalEnv
    New-Item -ItemType Directory -Path $context.GalConfigRoot -Force | Out-Null
    if (-not (Test-Path $localEnv) -and (Test-Path $legacyLocalEnv)) {
        $localEnv = $legacyLocalEnv
        Write-Host "  [LEGACY] Using existing legacy config.local.env at $localEnv" -ForegroundColor Yellow
    }

    if (-not (Test-Path $localEnv)) {
        if (Test-Path $exampleEnv) {
            Copy-Item $exampleEnv $localEnv
            Write-Host "  [OK] Created $localEnv from config.example.env"
            Write-Host "  [ACTION REQUIRED] Edit $localEnv with your paths" -ForegroundColor Yellow
        }
        else {
            Write-Host '  [WARN] config.example.env not found — skipping' -ForegroundColor Yellow
        }
    }
    else {
        Write-Host "  [SKIP] $localEnv already exists"
    }

    $exampleRoles = Join-Path $context.RepoRoot 'model-roles.example.md'
    $primaryLocalRoles = Join-Path $context.GalConfigRoot 'model-roles.local.md'
    $legacyLocalRoles = Join-Path $context.RepoRoot 'model-roles.local.md'
    $localRoles = $primaryLocalRoles
    if (-not (Test-Path $localRoles) -and (Test-Path $legacyLocalRoles)) {
        $localRoles = $legacyLocalRoles
        Write-Host "  [LEGACY] Using existing legacy model-roles.local.md at $localRoles" -ForegroundColor Yellow
    }

    if (-not (Test-Path $localRoles)) {
        if (Test-Path $exampleRoles) {
            Copy-Item $exampleRoles $localRoles
            Write-Host "  [OK] Created $localRoles from model-roles.example.md"
        }
    }
    else {
        Write-Host "  [SKIP] $localRoles already exists"
    }

    $exampleRouting = Join-Path $context.RepoRoot 'executor-routing.example.json'
    $localRouting   = Join-Path $context.GalConfigRoot 'executor-routing.json'
    $legacyRouting  = Join-Path $context.GalConfigRoot 'executor-routing.ndjson'

    if ((Test-Path $legacyRouting) -and -not (Test-Path $localRouting)) {
        Write-Host "  [MIGRATE] Old executor-routing.ndjson found at $legacyRouting but no .json present." -ForegroundColor Yellow
        Write-Host "  [MIGRATE] Convert: copy $legacyRouting to $localRouting using the role-keyed JSON schema in executor-routing.example.json" -ForegroundColor Yellow
    }

    if (-not (Test-Path $localRouting)) {
        if (Test-Path $exampleRouting) {
            New-Item -ItemType Directory -Path $context.GalConfigRoot -Force | Out-Null
            Copy-Item $exampleRouting $localRouting
            Write-Host "  [OK] Created $localRouting from executor-routing.example.json"
            Write-Host "  [ACTION REQUIRED] Edit $localRouting to map roles to your preferred executors" -ForegroundColor Yellow
        }
        else {
            Write-Host '  [WARN] executor-routing.example.json not found — skipping' -ForegroundColor Yellow
        }
    }
    else {
        Write-Host "  [SKIP] $localRouting already exists"
    }

    Push-Location $context.RepoRoot
    try {
        git config filter.gal-config.smudge 'bash scripts/gal-smudge.sh'
        git config filter.gal-config.clean 'bash scripts/gal-clean.sh'
        git config filter.gal-config.required true
        Write-Host "  [OK] Registered git filter 'gal-config' (smudge/clean)"

        git config core.hooksPath .githooks
        Write-Host '  [OK] Set core.hooksPath to .githooks'

        if (Test-Path $localEnv) {
            $hasValues = Get-Content $localEnv | Where-Object {
                $_ -notmatch '^\s*#' -and $_ -match '=.+ '
            }
            if (-not $hasValues) {
                $hasValues = Get-Content $localEnv | Where-Object {
                    $_ -notmatch '^\s*#' -and $_ -match '=.+'
                }
            }

            if ($hasValues) {
                $trackedFilterFiles = @('config.local.env', 'model-roles.local.md') | Where-Object {
                    git ls-files --error-unmatch $_ *> $null
                    $LASTEXITCODE -eq 0
                }

                if ($trackedFilterFiles.Count -gt 0) {
                    git checkout -- @trackedFilterFiles
                    Write-Host '  [OK] Re-checked out tracked filtered files (smudge filter applied)'
                }
            }
            else {
                Write-Host "  [INFO] $localEnv has no values yet — fill it in, then run: git checkout -- config.local.env model-roles.local.md" 
            }
        }
    }
    finally {
        Pop-Location
    }
}

Initialize-SetupSession -EntryScriptPath $MyInvocation.MyCommand.Path -Uninstall:$Uninstall -Replace:$Replace -DryRun:$DryRun -Reconfigure:$Reconfigure -SelectedRuntimes $SelectedRuntimes -PrimaryRuntime $PrimaryRuntime | Out-Null
Invoke-UpdatePersonalization