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

Write-Host 'Install mode resolution tests passed.'
