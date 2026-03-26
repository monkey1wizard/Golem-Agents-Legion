param(
    [string]$TargetPath = (Get-Location).Path,
    [string]$ProjectName,
    [switch]$Force
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptRoot

if (-not (Test-Path $TargetPath)) {
    throw "Target path does not exist: $TargetPath"
}

$resolvedTarget = (Resolve-Path $TargetPath).Path
if (-not $ProjectName) {
    $ProjectName = Split-Path $resolvedTarget -Leaf
}

$devDir = Join-Path $resolvedTarget ".dev"
$plansDir = Join-Path $resolvedTarget "docs\plans"
$projectTemplatePath = Join-Path $repoRoot "templates\project.md"
$stateTemplatePath = Join-Path $repoRoot "templates\state.md"

if (-not (Test-Path $projectTemplatePath)) {
    throw "Missing template: $projectTemplatePath"
}

if (-not (Test-Path $stateTemplatePath)) {
    throw "Missing template: $stateTemplatePath"
}

New-Item -ItemType Directory -Force -Path $devDir | Out-Null
New-Item -ItemType Directory -Force -Path $plansDir | Out-Null

$projectTargetPath = Join-Path $devDir "project.md"
$stateTargetPath = Join-Path $devDir "state.md"

if ((Test-Path $projectTargetPath) -and -not $Force) {
    throw "File already exists: $projectTargetPath. Use -Force to overwrite."
}

if ((Test-Path $stateTargetPath) -and -not $Force) {
    throw "File already exists: $stateTargetPath. Use -Force to overwrite."
}

$projectContent = Get-Content -Path $projectTemplatePath -Raw
$projectContent = $projectContent -replace "# \[Project Name\]", "# $ProjectName"

$stateContent = Get-Content -Path $stateTemplatePath -Raw

Set-Content -Path $projectTargetPath -Value $projectContent
Set-Content -Path $stateTargetPath -Value $stateContent

Write-Host "Initialized repo context in: $resolvedTarget"
Write-Host "- Created: .dev/project.md"
Write-Host "- Created: .dev/state.md"
Write-Host "- Ensured: docs/plans/"