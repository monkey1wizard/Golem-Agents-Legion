<#
.SYNOPSIS
    Creates symlinks from golem-agents-legion repo to ~/.copilot/ runtime directories.

.DESCRIPTION
    Links:
      - agent/*.agent.md  → ~/.copilot/agents/*.agent.md
      - skills/*/         → ~/.copilot/skills/*/

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
    foreach ($dir in @($copilotRoot, $agentsTarget, $skillsTarget)) {
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

# --- Summary ---

Write-Host ""
if ($Uninstall) {
    Write-Host "Uninstall complete."
}
elseif ($DryRun) {
    Write-Host "Dry run complete. No changes made."
}
else {
    Write-Host "Setup complete: agents=$agentOk/$($agentFiles.Count), skills=$skillOk/$($skillDirs.Count)"
    if ($agentFail -gt 0 -or $skillFail -gt 0) {
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

        # 5. Re-checkout filtered files to trigger smudge
        if (Test-Path $localEnv) {
            $hasValues = Get-Content $localEnv | Where-Object {
                $_ -notmatch '^\s*#' -and $_ -match '=.+'
            }
            if ($hasValues) {
                git checkout -- .
                Write-Host "  [OK] Re-checked out files (smudge filter applied)"
            }
            else {
                Write-Host "  [INFO] config.local.env has no values yet — fill it in, then run: git checkout -- ."
            }
        }
    }
    finally {
        Pop-Location
    }
}
