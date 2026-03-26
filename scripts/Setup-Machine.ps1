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
