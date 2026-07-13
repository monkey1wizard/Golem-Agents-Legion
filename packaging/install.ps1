# GAL — Golem Agents Legion installer (Windows)
# Usage: irm https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.ps1 | iex
#
# [BETA] Windows installation has not yet been validated on a clean machine.
# Please report any issues at: https://github.com/monkey1wizard/golem-agents-legion/issues
#
# Integrity model:
#   - SHA-256 check against checksums.txt is MANDATORY (aborts on mismatch)
#   - cosign verify-blob is BEST-EFFORT (warns if cosign absent, never blocks)
#
# Non-interactive: fully non-interactive (stdin may be a pipe). No prompts.
# Beta notice written to stderr as plain text.

#Requires -Version 5.1
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$Repo   = 'monkey1wizard/golem-agents-legion'
# Canonical-case slug — the cosign certificate SAN identity is case-SENSITIVE and
# carries the repository's canonical case; a lowercase regexp silently never matches.
$RepoCanonical = 'monkey1wizard/Golem-Agents-Legion'
# Flat layout home: gal.exe + source payload (skills/agents/commands/…) live side
# by side so resolve_source_from_exe_dir finds the source root next to the binary.
$InstallDir = Join-Path $env:LOCALAPPDATA 'Programs\gal'

function Write-Info  { param([string]$Msg) Write-Host "gal-install: $Msg" }
function Write-Warn  { param([string]$Msg) Write-Error "gal-install: warning: $Msg" -ErrorAction Continue }
function Write-Fatal { param([string]$Msg) Write-Error "gal-install: error: $Msg" -ErrorAction Stop; exit 1 }

# Beta notice — plain text to stderr, non-interactive
Write-Warn '[BETA] Windows support has not yet been validated on a clean machine.'
Write-Warn 'Proceeding with installation. Report issues at: https://github.com/monkey1wizard/golem-agents-legion/issues'

# --- detect architecture -----------------------------------------------------

function Get-GalArch {
    switch ($env:PROCESSOR_ARCHITECTURE) {
        'AMD64'  { return 'x64'   }
        'ARM64'  { return 'arm64' }
        default  {
            Write-Fatal "Unsupported architecture: $($env:PROCESSOR_ARCHITECTURE). Supported: AMD64, ARM64.`nPlease download the binary manually from: https://github.com/$Repo/releases"
        }
    }
}

# --- fetch latest version tag ------------------------------------------------

function Get-LatestVersion {
    try {
        $response = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -UseBasicParsing
        if (-not $response.tag_name) { throw 'empty tag_name' }
        return $response.tag_name
    } catch {
        Write-Fatal "Failed to fetch latest release version from GitHub API: $_"
    }
}

# --- SHA-256 helper ----------------------------------------------------------

function Get-FileSha256 {
    param([string]$Path)
    $hash = Get-FileHash -Path $Path -Algorithm SHA256
    return $hash.Hash.ToLower()
}

# --- main --------------------------------------------------------------------

$Arch    = Get-GalArch
$Platform = 'windows'
Write-Info "Detected platform: $Platform-$Arch"

$Version = if ($env:GAL_VERSION) { $env:GAL_VERSION } else { Get-LatestVersion }
Write-Info "Installing GAL $Version"

$ArchiveName = "gal-$Version-$Platform-$Arch.zip"
$BaseUrl     = "https://github.com/$Repo/releases/download/$Version"

$TmpDir = Join-Path $env:TEMP "gal-install-$(New-Guid)"
New-Item -ItemType Directory -Path $TmpDir -Force | Out-Null

try {
    # Download archive
    Write-Info "Downloading $ArchiveName..."
    $ArchivePath = Join-Path $TmpDir $ArchiveName
    try {
        Invoke-WebRequest -Uri "$BaseUrl/$ArchiveName" -OutFile $ArchivePath -UseBasicParsing
    } catch {
        Write-Fatal "Download failed: $BaseUrl/$ArchiveName`n$_"
    }

    # Download checksums.txt
    Write-Info 'Downloading checksums.txt...'
    $ChecksumsPath = Join-Path $TmpDir 'checksums.txt'
    try {
        Invoke-WebRequest -Uri "$BaseUrl/checksums.txt" -OutFile $ChecksumsPath -UseBasicParsing
    } catch {
        Write-Fatal "Download failed: $BaseUrl/checksums.txt`n$_"
    }

    # --- Mandatory SHA-256 verification --------------------------------------
    Write-Info 'Verifying SHA-256 integrity...'
    $ChecksumLines = Get-Content $ChecksumsPath
    $ExpectedLine  = $ChecksumLines | Where-Object { $_ -match [regex]::Escape($ArchiveName) } | Select-Object -First 1
    if (-not $ExpectedLine) {
        Write-Fatal "Checksum entry not found for '$ArchiveName' in checksums.txt."
    }
    $Expected = ($ExpectedLine -split '\s+')[0].ToLower()
    $Actual   = Get-FileSha256 $ArchivePath

    if ($Expected -ne $Actual) {
        Write-Fatal "SHA-256 mismatch — download may be corrupted or tampered.`n  Expected: $Expected`n  Actual:   $Actual`nAborting installation."
    }
    Write-Info "SHA-256 OK ($($Actual.Substring(0,16))...)"

    # --- cosign best-effort verification -------------------------------------
    $CosignExe = Get-Command 'cosign' -ErrorAction SilentlyContinue
    if ($CosignExe) {
        Write-Info 'cosign found — downloading signature files...'
        $SigPath  = Join-Path $TmpDir 'checksums.txt.sig'
        $CertPath = Join-Path $TmpDir 'checksums.txt.pem'
        try {
            Invoke-WebRequest -Uri "$BaseUrl/checksums.txt.sig" -OutFile $SigPath  -UseBasicParsing
            Invoke-WebRequest -Uri "$BaseUrl/checksums.txt.pem" -OutFile $CertPath -UseBasicParsing
            & cosign verify-blob `
                --signature $SigPath `
                --certificate $CertPath `
                --certificate-identity-regexp "https://github.com/$RepoCanonical/\.github/workflows/release\.yml@.*" `
                --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' `
                $ChecksumsPath 2>$null
            if ($LASTEXITCODE -eq 0) {
                Write-Info 'cosign signature verified.'
            } else {
                Write-Warn 'cosign verification failed — proceeding (SHA-256 passed).'
            }
        } catch {
            Write-Warn "Could not complete cosign verification — skipping (SHA-256 passed)."
        }
    } else {
        Write-Warn 'cosign not found in PATH — skipping cosign verification (SHA-256 passed).'
        Write-Warn 'Install cosign from https://docs.sigstore.dev/cosign/system_config/installation/ for full verification.'
    }

    # --- Extract and install -------------------------------------------------
    Write-Info 'Extracting archive...'
    $ExtractDir = Join-Path $TmpDir 'extract'
    Expand-Archive -Path $ArchivePath -DestinationPath $ExtractDir -Force

    $BinaryName = 'gal.exe'
    $Binary = Get-ChildItem -Path $ExtractDir -Filter $BinaryName -Recurse | Select-Object -First 1
    if (-not $Binary) {
        Write-Fatal "Binary '$BinaryName' not found in archive."
    }

    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }
    # Flat install: place gal.exe together with the source payload
    # (skills/agents/commands/… + mcp.json) so `gal refresh` resolves a source
    # root next to the binary. Binary-only install left refresh failing with
    # "Source root not found". Clear any prior install for a clean upgrade, then
    # copy the whole extracted tree (gal.exe + payload) into the flat dir.
    Get-ChildItem -Path $InstallDir -Force | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
    Copy-Item -Path (Join-Path $ExtractDir '*') -Destination $InstallDir -Recurse -Force

    # --- Verify install ------------------------------------------------------
    $GalBin = Join-Path $InstallDir $BinaryName
    $GalVersionOut = & $GalBin --version 2>&1

    Write-Host ''
    Write-Host 'GAL installed successfully.'
    Write-Host "  Location: $GalBin"
    Write-Host "  Version:  $GalVersionOut"
    Write-Host ''

    # PATH reminder — append ONLY the GAL install dir to the User PATH.
    # (The old hint wrote the whole process PATH back into User scope = bloat,
    # and pointed at the previous install dir.)
    $UserPath = [System.Environment]::GetEnvironmentVariable('PATH', 'User')
    if ($UserPath -notlike "*$InstallDir*") {
        Write-Host "Add $InstallDir to your PATH (appends only the GAL dir to your User PATH):"
        Write-Host "  [Environment]::SetEnvironmentVariable('PATH', [Environment]::GetEnvironmentVariable('PATH','User') + ';$InstallDir', 'User')"
        Write-Host ''
    }

    Write-Host 'Next steps:'
    Write-Host '  gal refresh     -- project GAL onto your coding agents (run once after install)'
    Write-Host '  gal init        -- initialize GAL in a new repo'
    Write-Host '  gal --help      -- show all commands'
    Write-Host ''
    Write-Host "Documentation: https://github.com/$Repo"
    Write-Host ''
    Write-Host '[BETA] Please report any issues at: https://github.com/monkey1wizard/golem-agents-legion/issues' -ForegroundColor Yellow

} finally {
    Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
}
