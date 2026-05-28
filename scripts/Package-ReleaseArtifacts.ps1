<#
.SYNOPSIS
    Packages release artifacts for GAL, ensuring all fallbacks wrap the same canonical binary.

.DESCRIPTION
    This script is part of the GAL release flow (T-003). It takes a source binary, version string, 
    and target platform, then generates the canonical fallback archive (.zip for Windows, 
    .tar.gz for macOS/Linux) containing only the single executable binary and the LICENSE.
    It does NOT bundle ~/.gal state or any other files.
    Finally, it updates the checksums.txt file.

.EXAMPLE
    .\Package-ReleaseArtifacts.ps1 -SourceBinary ".\dist\gal.exe" -Version "v1.0.0" -TargetPlatform "windows" -TargetArch "x64"
#>

[CmdletBinding()]
param (
    [Parameter(Mandatory=$true)]
    [ValidateNotNullOrEmpty()]
    [string]$SourceBinary,

    [Parameter(Mandatory=$true)]
    [ValidateNotNullOrEmpty()]
    [string]$Version,

    [Parameter(Mandatory=$true)]
    [ValidateSet("windows", "darwin", "linux")]
    [string]$TargetPlatform,

    [Parameter(Mandatory=$true)]
    [ValidateSet("x64", "arm64")]
    [string]$TargetArch,

    [Parameter(Mandatory=$false)]
    [string]$OutputDir = (Join-Path $env:USERPROFILE '.gal\dist\release')
)

$ErrorActionPreference = "Stop"
$InformationPreference = "Continue"

if (-Not (Test-Path $SourceBinary)) {
    Write-Error "Source binary not found at $SourceBinary"
    exit 1
}

if (-Not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir | Out-Null
}

$resolvedOutputDir = (Resolve-Path $OutputDir).Path

$licenseFile = "LICENSE"
if (-Not (Test-Path $licenseFile)) {
    Write-Warning "LICENSE file not found in root. Artifacts must include a LICENSE."
    # We continue, but in a real CI this should probably fail.
}

# Define artifact names based on the matrix (T-002/T-003)
$normalizedVersion = $Version.Trim()

if ($TargetPlatform -eq "windows") {
    $binaryName = "gal-$normalizedVersion-$TargetPlatform-$TargetArch.exe"
    $archiveExt = ".zip"
} else {
    $binaryName = "gal-$normalizedVersion-$TargetPlatform-$TargetArch"
    $archiveExt = ".tar.gz"
}

$archiveName = "gal-$normalizedVersion-$TargetPlatform-$TargetArch$archiveExt"
$archivePath = Join-Path $resolvedOutputDir $archiveName
$stagingDir = Join-Path $resolvedOutputDir "staging-$TargetPlatform-$TargetArch"

Write-Information "Packaging $archiveName for $normalizedVersion..."

# Create clean staging
if (Test-Path $stagingDir) {
    Remove-Item -Path $stagingDir -Recurse -Force
}
New-Item -ItemType Directory -Path $stagingDir | Out-Null

# Copy canonical binary and license to staging
Copy-Item -Path $SourceBinary -Destination (Join-Path $stagingDir $binaryName) -Force
if (Test-Path $licenseFile) {
    Copy-Item -Path $licenseFile -Destination (Join-Path $stagingDir $licenseFile) -Force
}

# Create Archive
if ($archiveExt -eq ".zip") {
    Compress-Archive -Path "$stagingDir\*" -DestinationPath $archivePath -Force
} else {
    # Requires tar on Windows (available in recent Windows 10/11)
    $cwd = (Get-Location).Path
    Set-Location $stagingDir
    & tar -czf $archivePath *
    Set-Location $cwd
}

Write-Information "Created archive at $archivePath"

# Cleanup staging
Remove-Item -Path $stagingDir -Recurse -Force

# Generate / Update Checksums
$checksumFile = Join-Path $resolvedOutputDir "checksums.txt"
$hash = (Get-FileHash -Path $archivePath -Algorithm SHA256).Hash.ToLower()

$hashLine = "$hash  $archiveName"
if (Test-Path $checksumFile) {
    $remainingLines = Get-Content $checksumFile | Where-Object { $_ -notmatch "  $([Regex]::Escape($archiveName))$" -and $_ -notmatch "  $([Regex]::Escape($binaryName))$" }
    Set-Content -Path $checksumFile -Value $remainingLines
}
Add-Content -Path $checksumFile -Value $hashLine

# Also hash the bare binary if it's not already in the checksum file
$binaryPath = Join-Path $resolvedOutputDir $binaryName
Copy-Item -Path $SourceBinary -Destination $binaryPath -Force
$binaryHash = (Get-FileHash -Path $binaryPath -Algorithm SHA256).Hash.ToLower()
$binaryHashLine = "$binaryHash  $binaryName"
Add-Content -Path $checksumFile -Value $binaryHashLine

Write-Information "Updated $checksumFile"
Write-Information "Packaging complete for $TargetPlatform-$TargetArch."
