Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot '..\common\Common.ps1')

# TP-001: devMode=true + usable galRoot -> source
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$resolved = Resolve-InstallMode -devMode $true -galRoot $repoRoot
if ($resolved -ne 'source') {
    throw "Expected source mode when devMode=true and galRoot is usable, got '$resolved'"
}

# TP-004: devMode=false -> install (normal mode)
$bootstrap = Resolve-InstallMode -devMode $false -galRoot ''
if ($bootstrap -ne 'install') {
    throw "Expected install mode when devMode=false, got '$bootstrap'"
}

# Predicate: a minimal directory with commands/agent/skills is usable.
$minimalRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('gal-root-authority-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path (Join-Path $minimalRoot 'commands') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $minimalRoot 'agent') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $minimalRoot 'skills') -Force | Out-Null

if (-not (Test-GalRootUsable -GalRoot $minimalRoot)) {
    throw 'Expected minimal galRoot to be considered usable when it contains commands/agent/skills.'
}

# TP-003 (critical): devMode=true + UNUSABLE galRoot must THROW, never fall back.
$incompleteRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('gal-root-incomplete-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path (Join-Path $incompleteRoot 'commands') -Force | Out-Null  # missing agent/ + skills/

$threwOnMissingDirs = $false
try {
    Resolve-InstallMode -devMode $true -galRoot $incompleteRoot | Out-Null
}
catch {
    $threwOnMissingDirs = $true
    if ($_.Exception.Message -notmatch 'skills|agent') {
        throw "Expected the error to name the missing contract directory, got: $($_.Exception.Message)"
    }
}
if (-not $threwOnMissingDirs) {
    throw 'SILENT FALLBACK REGRESSION: devMode=true with an incomplete galRoot must throw, not fall back to install.'
}

# TP-003: devMode=true + nonexistent path must THROW.
$missingRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('gal-root-missing-' + [Guid]::NewGuid().ToString('N'))
$threwOnMissingPath = $false
try {
    Resolve-InstallMode -devMode $true -galRoot $missingRoot | Out-Null
}
catch {
    $threwOnMissingPath = $true
}
if (-not $threwOnMissingPath) {
    throw 'SILENT FALLBACK REGRESSION: devMode=true with a nonexistent galRoot must throw.'
}

# TP-003: devMode=true + empty galRoot must THROW.
$threwOnEmpty = $false
try {
    Resolve-InstallMode -devMode $true -galRoot '' | Out-Null
}
catch {
    $threwOnEmpty = $true
}
if (-not $threwOnEmpty) {
    throw 'SILENT FALLBACK REGRESSION: devMode=true with an empty galRoot must throw.'
}

# Get-GalRootUsableError names the failed check.
$reason = Get-GalRootUsableError -GalRoot $incompleteRoot
if ([string]::IsNullOrWhiteSpace($reason)) {
    throw 'Expected Get-GalRootUsableError to return a reason for an incomplete galRoot.'
}

Remove-Item -LiteralPath $minimalRoot -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $incompleteRoot -Recurse -Force -ErrorAction SilentlyContinue

Write-Host 'Install mode resolution tests passed.'
