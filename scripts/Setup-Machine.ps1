<#
.SYNOPSIS
    Creates symlinks from golem-agents-legion repo to Copilot, Gemini, and Codex runtime directories.

.DESCRIPTION
        Links:
            - agent/*.agent.md  → ~/.copilot/agents/*.agent.md
            - skills/*/         → ~/.copilot/skills/*/
            - skills/*/         → ~/.gemini/skills/*/ (Gemini CLI)
            - skills/*/         → ~/.agents/skills/*/ (Codex CLI)
            - commands/gal/     → ~/.copilot/skills/gal/ + ~/.gemini/skills/gal/ + ~/.agents/skills/gal/ (baked dispatcher skill)
            - <repo root>       → ~/.copilot/gal/ + ~/.gemini/gal/ (GAL_ROOT dir symlinks)
      - Generates commands/gal/SKILL.md from SKILL.template.md (baked absolute paths)
      - Generates ~/.gemini/gal-context.md (@file skill imports)

    On Windows, requires Developer Mode enabled or admin privileges for symlinks.
    Falls back to directory junctions for skill folders if symlinks fail.

.PARAMETER Uninstall
    Remove all symlinks created by this script instead of creating them.

.PARAMETER Replace
    When a real (non-link) directory/file already exists at the target, rename it
    to *.bak and create the symlink. Without this flag, existing items are skipped.

.PARAMETER DryRun
    Show what would be done without making changes.

.EXAMPLE
    .\scripts\Setup-Machine.ps1
    .\scripts\Setup-Machine.ps1 -Replace
    .\scripts\Setup-Machine.ps1 -DryRun
    .\scripts\Setup-Machine.ps1 -Uninstall
#>
param(
    [switch]$Uninstall,
    [switch]$Replace,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptRoot

$copilotRoot = Join-Path $env:USERPROFILE ".copilot"
$agentsTarget = Join-Path $copilotRoot "agents"
$skillsTarget = Join-Path $copilotRoot "skills"

$geminiRoot = Join-Path $env:USERPROFILE ".gemini"
$geminiSkillsTarget = Join-Path $geminiRoot "skills"
$geminiContextFile = Join-Path $geminiRoot "gal-context.md"

$codexSkillsRoot = Join-Path $env:USERPROFILE ".agents"
$codexSkillsTarget = Join-Path $codexSkillsRoot "skills"

$galSource       = Join-Path $repoRoot "commands\gal"
$galRootCopilot  = Join-Path $copilotRoot "gal"
$galRootGemini   = Join-Path $geminiRoot "gal"
$galSkillCopilot = Join-Path $skillsTarget "gal"
$galSkillGemini  = Join-Path $geminiSkillsTarget "gal"
$galSkillCodex   = Join-Path $codexSkillsTarget "gal"
$skillTemplate   = Join-Path $galSource "SKILL.template.md"
$commandsSourceDir = Join-Path $repoRoot "commands"
$commandAliasNames = Get-ChildItem $commandsSourceDir -Directory |
    Where-Object { $_.Name -ne 'gal' } |
    Select-Object -ExpandProperty Name

$commandSkillDirs = @(
    [pscustomobject]@{
        Name          = 'gal'
        Source        = $galSource
        Template      = $skillTemplate
        CopilotTarget = $galSkillCopilot
        GeminiTarget  = $galSkillGemini
        CodexTarget   = $galSkillCodex
    }
) + ($commandAliasNames | ForEach-Object {
    $source = Join-Path $repoRoot ("commands\{0}" -f $_)
    [pscustomobject]@{
        Name          = $_
        Source        = $source
        Template      = Join-Path $source "SKILL.template.md"
        CopilotTarget = Join-Path $skillsTarget $_
        GeminiTarget  = Join-Path $geminiSkillsTarget $_
        CodexTarget   = Join-Path $codexSkillsTarget $_
    }
})
$activeCommandSkillNames = $commandSkillDirs | ForEach-Object { $_.Name }

# --- Helpers ---

function Test-SymlinkOrJunction([string]$Path) {
    if (-not (Test-Path $Path)) { return $false }
    $item = Get-Item $Path -Force
    return ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0
}

function New-SafeSymlink([string]$LinkPath, [string]$TargetPath, [string]$Type) {
    if ($DryRun) {
        Write-Host "  [DRY RUN] $Type link: $LinkPath -> $TargetPath"
        return $true
    }

    # If a symlink/junction already points to the correct target, skip
    if (Test-SymlinkOrJunction $LinkPath) {
        $existing = Get-Item $LinkPath -Force
        if ($existing.Target -contains $TargetPath) {
            Write-Host "  [SKIP] Already linked: $LinkPath"
            return $true
        }
        # Points elsewhere — remove and re-create
        Write-Host "  [UPDATE] Replacing existing link: $LinkPath"
        $existing.Delete()
    }
    elseif (Test-Path $LinkPath) {
        if ($Replace) {
            $bakPath = "$LinkPath.bak"
            if (Test-Path $bakPath) {
                Write-Host "  [WARN] Backup already exists at $bakPath — skipping"
                return $false
            }
            if ($DryRun) {
                Write-Host "  [DRY RUN] Would rename $LinkPath -> $bakPath, then link"
                return $true
            }
            Rename-Item $LinkPath $bakPath
            Write-Host "  [BACKUP] $LinkPath -> $bakPath"
        }
        else {
            Write-Host "  [WARN] Non-link item exists at $LinkPath — skipping (use -Replace to back up and link)"
            return $false
        }
    }

    try {
        if ($Type -eq "File") {
            New-Item -ItemType SymbolicLink -Path $LinkPath -Target $TargetPath -Force | Out-Null
        }
        else {
            # Try symlink first, fall back to junction on permission error
            try {
                New-Item -ItemType SymbolicLink -Path $LinkPath -Target $TargetPath -Force | Out-Null
            }
            catch {
                Write-Host "  [FALLBACK] Symlink failed, using junction: $LinkPath"
                New-Item -ItemType Junction -Path $LinkPath -Target $TargetPath -Force | Out-Null
            }
        }
        Write-Host "  [OK] $LinkPath -> $TargetPath"
        return $true
    }
    catch {
        Write-Host "  [ERROR] Failed to create link: $LinkPath — $_"
        return $false
    }
}

function Remove-SafeLink([string]$LinkPath) {
    if (-not (Test-SymlinkOrJunction $LinkPath)) { return }

    if ($DryRun) {
        Write-Host "  [DRY RUN] Would remove: $LinkPath"
        return
    }

    (Get-Item $LinkPath -Force).Delete()
    Write-Host "  [REMOVED] $LinkPath"
}

# --- Ensure target directories ---

if (-not $Uninstall) {
    foreach ($dir in @($copilotRoot, $agentsTarget, $skillsTarget, $geminiRoot, $geminiSkillsTarget, $codexSkillsRoot, $codexSkillsTarget)) {
        if (-not (Test-Path $dir)) {
            if ($DryRun) {
                Write-Host "[DRY RUN] Would create directory: $dir"
            }
            else {
                New-Item -ItemType Directory -Path $dir -Force | Out-Null
                Write-Host "[OK] Created directory: $dir"
            }
        }
    }
}

# --- Agent symlinks (file-level: *.agent.md) ---

$agentSourceDir = Join-Path $repoRoot "agent"
$agentFiles = Get-ChildItem $agentSourceDir -Filter "*.agent.md" -File

Write-Host ""
Write-Host "=== Agents ($($agentFiles.Count) files) ==="

$agentOk = 0
$agentFail = 0

foreach ($f in $agentFiles) {
    $linkPath = Join-Path $agentsTarget $f.Name

    if ($Uninstall) {
        Remove-SafeLink $linkPath
    }
    else {
        if (New-SafeSymlink $linkPath $f.FullName "File") { $agentOk++ } else { $agentFail++ }
    }
}

# --- Skill symlinks (directory-level: each skill folder) ---

$skillSourceDir = Join-Path $repoRoot "skills"
$skillDirs = Get-ChildItem $skillSourceDir -Directory

Write-Host ""
Write-Host "=== Skills ($($skillDirs.Count) directories) ==="

$skillOk = 0
$skillFail = 0

foreach ($d in $skillDirs) {
    $linkPath = Join-Path $skillsTarget $d.Name

    if ($Uninstall) {
        Remove-SafeLink $linkPath
    }
    else {
        if (New-SafeSymlink $linkPath $d.FullName "Directory") { $skillOk++ } else { $skillFail++ }
    }
}

# --- Gemini Skill symlinks (directory-level: same skills, different target) ---

Write-Host ""
Write-Host "=== Gemini Skills ($($skillDirs.Count) directories) ==="

$geminiSkillOk = 0
$geminiSkillFail = 0

foreach ($d in $skillDirs) {
    $linkPath = Join-Path $geminiSkillsTarget $d.Name

    if ($Uninstall) {
        Remove-SafeLink $linkPath
    }
    else {
        if (New-SafeSymlink $linkPath $d.FullName "Directory") { $geminiSkillOk++ } else { $geminiSkillFail++ }
    }
}

# --- Codex Skill symlinks (directory-level: same skills, different target) ---

Write-Host ""
Write-Host "=== Codex Skills ($($skillDirs.Count) directories) ==="

$codexSkillOk = 0
$codexSkillFail = 0

foreach ($d in $skillDirs) {
    $linkPath = Join-Path $codexSkillsTarget $d.Name

    if ($Uninstall) {
        Remove-SafeLink $linkPath
    }
    else {
        if (New-SafeSymlink $linkPath $d.FullName "Directory") { $codexSkillOk++ } else { $codexSkillFail++ }
    }
}

# --- Gemini gal-context.md ---

Write-Host ""
Write-Host "=== Gemini gal-context.md ==="

if ($Uninstall) {
    if (Test-Path $geminiContextFile) {
        if ($DryRun) {
            Write-Host "  [DRY RUN] Would remove: $geminiContextFile"
        }
        else {
            Remove-Item $geminiContextFile -Force
            Write-Host "  [REMOVED] $geminiContextFile"
        }
    }
}
else {
    $skillImports = $skillDirs | Sort-Object Name | ForEach-Object {
        $skillMd = Join-Path $geminiSkillsTarget $_.Name "SKILL.md"
        "@$skillMd"
    }
    $commandSkillImports = $commandSkillDirs | ForEach-Object {
        "@" + (Join-Path $_.GeminiTarget "SKILL.md")
    }
    $allImports = $commandSkillImports + $skillImports
    $contextContent = ($allImports -join "`n") + "`n"

    if ($DryRun) {
        Write-Host "  [DRY RUN] Would write: $geminiContextFile ($($skillDirs.Count + $commandSkillDirs.Count) skill imports)"
    }
    else {
        $utf8NoBom = New-Object System.Text.UTF8Encoding $false
        [System.IO.File]::WriteAllText($geminiContextFile, $contextContent, $utf8NoBom)
        Write-Host "  [OK] $geminiContextFile ($($skillDirs.Count + $commandSkillDirs.Count) skill imports)"
    }
}

# --- Gemini settings.json: context.fileName bridge ---

$geminiSettingsFile = Join-Path $geminiRoot "settings.json"

Write-Host ""
Write-Host "=== Gemini settings.json bridge ==="

if ($Uninstall) {
    Write-Host "  [SKIP] settings.json not modified during uninstall (user-owned file)"
} elseif ($DryRun) {
    Write-Host "  [DRY RUN] Would merge AGENTS.md into context.fileName in: $geminiSettingsFile"
} else {
    if (Test-Path $geminiSettingsFile) {
        $rawJson = Get-Content $geminiSettingsFile -Raw -Encoding UTF8
        try   { $settings = $rawJson | ConvertFrom-Json }
        catch { Write-Host "  [WARN] Could not parse $geminiSettingsFile as JSON — skipping bridge" -ForegroundColor Yellow; $settings = $null }
    } else {
        $settings = [PSCustomObject]@{}
    }

    if ($null -ne $settings) {
        if (-not (Get-Member -InputObject $settings -Name 'context' -MemberType NoteProperty)) {
            Add-Member -InputObject $settings -MemberType NoteProperty -Name 'context' -Value ([PSCustomObject]@{})
        }
        if (-not (Get-Member -InputObject $settings.context -Name 'fileName' -MemberType NoteProperty)) {
            Add-Member -InputObject $settings.context -MemberType NoteProperty -Name 'fileName' -Value @('AGENTS.md', 'GEMINI.md')
        } else {
            $current = @($settings.context.fileName)
            foreach ($required in @('AGENTS.md', 'GEMINI.md')) {
                if ($current -notcontains $required) { $current += $required }
            }
            $settings.context.fileName = $current
        }
        $utf8NoBom = New-Object System.Text.UTF8Encoding $false
        [System.IO.File]::WriteAllText($geminiSettingsFile, ($settings | ConvertTo-Json -Depth 10), $utf8NoBom)
        Write-Host "  [OK] $geminiSettingsFile (context.fileName includes AGENTS.md and GEMINI.md)"
    }
}

$galRootOk = 0
Write-Host ""
Write-Host "=== GAL_ROOT symlinks ==="

if ($Uninstall) {
    Remove-SafeLink $galRootCopilot
    Remove-SafeLink $galRootGemini
} else {
    if (New-SafeSymlink $galRootCopilot $repoRoot "Directory") { $galRootOk++ }
    if (New-SafeSymlink $galRootGemini  $repoRoot "Directory") { $galRootOk++ }
}

# --- Generated GAL command skills (bake templates → commands/gal*/SKILL.md) ---

Write-Host ""
Write-Host "=== Generated GAL command skills ==="

if ($Uninstall) {
    foreach ($commandSkill in $commandSkillDirs) {
        $bakedSkill = Join-Path $commandSkill.Source "SKILL.md"
        if (Test-Path $bakedSkill) {
            if ($DryRun) { Write-Host "  [DRY RUN] Would remove baked: $bakedSkill" }
            else { Remove-Item $bakedSkill -Force; Write-Host "  [REMOVED] $bakedSkill" }
        }
    }
} else {
    foreach ($commandSkill in $commandSkillDirs) {
        if (-not (Test-Path $commandSkill.Template)) {
            Write-Host "  [WARN] Template not found: $($commandSkill.Template)"
        } else {
            $baked = (Get-Content $commandSkill.Template -Raw) -replace [regex]::Escape('{{GAL_ROOT}}'), $repoRoot
            $bakedSkill = Join-Path $commandSkill.Source "SKILL.md"
            if ($DryRun) {
                Write-Host "  [DRY RUN] Would write baked: $bakedSkill"
            } else {
                $utf8NoBom = New-Object System.Text.UTF8Encoding $false
                [System.IO.File]::WriteAllText($bakedSkill, $baked, $utf8NoBom)
                Write-Host "  [OK] $bakedSkill"
            }
        }
    }
}

# --- GAL command skill symlinks (commands/gal*/ → ~/.copilot/skills/gal*/ + ~/.gemini/skills/gal*/ + ~/.agents/skills/gal*/) ---

Write-Host ""
Write-Host "=== GAL command skill symlinks ==="

if ($Uninstall) {
    foreach ($commandSkill in $commandSkillDirs) {
        Remove-SafeLink $commandSkill.CopilotTarget
        Remove-SafeLink $commandSkill.GeminiTarget
        Remove-SafeLink $commandSkill.CodexTarget
    }
} else {
    foreach ($commandSkill in $commandSkillDirs) {
        New-SafeSymlink $commandSkill.CopilotTarget $commandSkill.Source "Directory" | Out-Null
        New-SafeSymlink $commandSkill.GeminiTarget  $commandSkill.Source "Directory" | Out-Null
        New-SafeSymlink $commandSkill.CodexTarget   $commandSkill.Source "Directory" | Out-Null
    }
}

# --- Migration: remove legacy gal-* dirs from installed locations ---

Write-Host ""
Write-Host "=== Migration: gal-* cleanup ==="

foreach ($skillsDir in @($skillsTarget, $geminiSkillsTarget, $codexSkillsTarget)) {
    $legacyDirs = Get-ChildItem $skillsDir -Directory -ErrorAction SilentlyContinue | Where-Object {
        $_.Name -like 'gal-*' -and $_.Name -notin $activeCommandSkillNames
    }
    foreach ($d in $legacyDirs) {
        if ($DryRun) {
            Write-Host "  [DRY RUN] Would remove: $($d.FullName)"
        } else {
            Remove-Item $d.FullName -Recurse -Force
            Write-Host "  [REMOVED] $($d.FullName)"
        }
    }
}

# --- Summary ---

Write-Host ""
if ($Uninstall) {
    Write-Host "Uninstall complete."
}
elseif ($DryRun) {
    Write-Host "Dry run complete. No changes made."
}
else {
    Write-Host "Setup complete: agents=$agentOk/$($agentFiles.Count), skills(copilot)=$skillOk/$($skillDirs.Count), skills(gemini)=$geminiSkillOk/$($skillDirs.Count), skills(codex)=$codexSkillOk/$($skillDirs.Count), gal-root=$galRootOk/2"
    Write-Host "Note: If SKILL.template.md changes, re-run Setup-Machine.ps1 -Replace to regenerate."
    if ($agentFail -gt 0 -or $skillFail -gt 0 -or $geminiSkillFail -gt 0 -or $codexSkillFail -gt 0) {
        Write-Host "Some links failed. Check warnings above." -ForegroundColor Yellow
        Write-Host "Tip: Enable Developer Mode in Windows Settings > Privacy & Security > For Developers" -ForegroundColor Yellow
    }
}

# --- Personalization: config.local.env + smudge/clean filter ---

if (-not $Uninstall -and -not $DryRun) {
    Write-Host ""
    Write-Host "=== Personalization ==="

    # 1. Copy config.example.env → config.local.env if missing
    $exampleEnv = Join-Path $repoRoot "config.example.env"
    $localEnv = Join-Path $repoRoot "config.local.env"

    if (-not (Test-Path $localEnv)) {
        if (Test-Path $exampleEnv) {
            Copy-Item $exampleEnv $localEnv
            Write-Host "  [OK] Created config.local.env from config.example.env"
            Write-Host "  [ACTION REQUIRED] Edit config.local.env with your paths" -ForegroundColor Yellow
        }
        else {
            Write-Host "  [WARN] config.example.env not found — skipping" -ForegroundColor Yellow
        }
    }
    else {
        Write-Host "  [SKIP] config.local.env already exists"
    }

    # 2. Copy model-roles.example.md → model-roles.local.md if missing
    $exampleRoles = Join-Path $repoRoot "model-roles.example.md"
    $localRoles = Join-Path $repoRoot "model-roles.local.md"

    if (-not (Test-Path $localRoles)) {
        if (Test-Path $exampleRoles) {
            Copy-Item $exampleRoles $localRoles
            Write-Host "  [OK] Created model-roles.local.md from model-roles.example.md"
        }
    }
    else {
        Write-Host "  [SKIP] model-roles.local.md already exists"
    }

    # 3. Register git smudge/clean filter (uses bash from Git for Windows)
    Push-Location $repoRoot
    try {
        git config filter.gal-config.smudge "bash scripts/gal-smudge.sh"
        git config filter.gal-config.clean  "bash scripts/gal-clean.sh"
        git config filter.gal-config.required true
        Write-Host "  [OK] Registered git filter 'gal-config' (smudge/clean)"

        # 4. Set custom hooks path
        git config core.hooksPath .githooks
        Write-Host "  [OK] Set core.hooksPath to .githooks"

        # 5. Re-checkout ONLY the smudge-filtered files (not all tracked files)
        if (Test-Path $localEnv) {
            $hasValues = Get-Content $localEnv | Where-Object {
                $_ -notmatch '^\s*#' -and $_ -match '=.+'
            }
            if ($hasValues) {
                $trackedFilterFiles = @(
                    "config.local.env",
                    "model-roles.local.md"
                ) | Where-Object {
                    (git ls-files --error-unmatch $_ 2>$null) -ne $null
                }

                if ($trackedFilterFiles.Count -gt 0) {
                    git checkout -- @trackedFilterFiles
                    Write-Host "  [OK] Re-checked out tracked filtered files (smudge filter applied)"
                }
                else {
                    Write-Host "  [INFO] Filtered files are not tracked yet — skipping git checkout"
                }
            }
            else {
                Write-Host "  [INFO] config.local.env has no values yet — fill it in, then run: git checkout -- config.local.env model-roles.local.md"
            }
        }
    }
    finally {
        Pop-Location
    }
}
