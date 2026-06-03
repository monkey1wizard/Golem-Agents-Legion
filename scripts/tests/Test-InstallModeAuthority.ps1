Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot '..\common\common.ps1')

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$resolved = Resolve-InstallMode -devMode $true -galRoot $repoRoot
if ($resolved -ne 'source') {
    throw "Expected source mode when devMode=true and galRoot is usable, got '$resolved'"
}

$bootstrap = Resolve-InstallMode -devMode $false -galRoot ''
if ($bootstrap -ne 'install') {
    throw "Expected install mode for bootstrap install, got '$bootstrap'"
}

$minimalRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('gal-root-authority-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path (Join-Path $minimalRoot 'commands') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $minimalRoot 'agent') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $minimalRoot 'skills') -Force | Out-Null

if (-not (Test-GalRootUsable -GalRoot $minimalRoot)) {
    throw 'Expected minimal galRoot to be considered usable when it contains commands/agent/skills.'
}

Remove-Item -LiteralPath $minimalRoot -Recurse -Force -ErrorAction SilentlyContinue

Write-Host 'Install mode resolution tests passed.'
