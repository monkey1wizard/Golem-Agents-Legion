param(
    [string]$TargetPath = (Get-Location).Path,
    [string]$ProjectName,
    [switch]$Blank,
    [switch]$Force
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptRoot
$graphifyVersionFileName = 'GAL_GRAPHIFY_VERSION.txt'

function Test-CommandAvailable([string]$Name) {
    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Get-GraphifyVersion {
    try {
        $versionOutput = & graphify --version 2>$null
        if ($LASTEXITCODE -eq 0 -and $versionOutput) {
            return ($versionOutput | Select-Object -First 1).Trim()
        }
    }
    catch {
    }

    return $null
}

function Write-GraphifyVersionStamp([string]$RepoPath, [string]$Version) {
    $graphVersionPath = Join-Path $RepoPath (Join-Path 'graphify-out' $graphifyVersionFileName)
    Set-Content -Path $graphVersionPath -Value $Version
}

function Invoke-GraphifyAutoInit([string]$RepoPath) {
    $graphReportPath = Join-Path $RepoPath "graphify-out\GRAPH_REPORT.md"
    $graphVersionPath = Join-Path $RepoPath (Join-Path 'graphify-out' $graphifyVersionFileName)
    if (-not (Test-Path $graphReportPath)) {
        Write-Host "- Skipped: graphify artifact inspection (no graphify-out/GRAPH_REPORT.md)"
        return
    }

    Write-Host "- Detected: existing graphify-out/GRAPH_REPORT.md"

    if ((Test-Path $graphVersionPath) -and (Test-CommandAvailable 'graphify')) {
        $currentVersion = Get-GraphifyVersion
        if ($currentVersion) {
            $stampedVersion = (Get-Content -Path $graphVersionPath -ErrorAction SilentlyContinue | Select-Object -First 1)
            $reportInfo = Get-Item $graphReportPath
            $stampInfo = Get-Item $graphVersionPath
            if ($stampedVersion -and $stampedVersion.Trim() -ne $currentVersion -and $reportInfo.LastWriteTimeUtc -le $stampInfo.LastWriteTimeUtc) {
                Write-Warning "graphify version changed (report: $($stampedVersion.Trim()), installed: $currentVersion). Refresh graphify artifacts manually if you want updated graph context."
            }
        }
    }

    if (-not (Test-Path $graphVersionPath) -and (Test-CommandAvailable 'graphify')) {
        $currentVersion = Get-GraphifyVersion
        if ($currentVersion) {
            Write-GraphifyVersionStamp -RepoPath $RepoPath -Version $currentVersion
            Write-Host "- Stamped: graphify-out/$graphifyVersionFileName ($currentVersion)"
        }
    }

    Write-Host "- Skipped: graphify auto-run"
}

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

# --- Adopt-existing: scan for existing documentation ---
$sourceDocs = @()
$techHints = @()

if (-not $Blank) {
    # Scan for README variants
    foreach ($name in @("README.md", "README", "README.rst", "README.txt")) {
        $p = Join-Path $resolvedTarget $name
        if (Test-Path $p) { $sourceDocs += @{ Path = $name; Type = "README" } }
    }

    # Scan docs/ directory
    $docsDir = Join-Path $resolvedTarget "docs"
    if (Test-Path $docsDir) {
        Get-ChildItem -Path $docsDir -File -Recurse -Include "*.md","*.rst","*.txt" | ForEach-Object {
            $rel = $_.FullName.Substring($resolvedTarget.Length + 1) -replace "\\", "/"
            $sourceDocs += @{ Path = $rel; Type = "docs" }
        }
    }

    # Scan for ADR
    foreach ($adrDir in @("docs/adr", "docs/ADR", "adr", "ADR")) {
        $p = Join-Path $resolvedTarget $adrDir
        if (Test-Path $p) {
            Get-ChildItem -Path $p -File -Recurse -Include "*.md" | ForEach-Object {
                $rel = $_.FullName.Substring($resolvedTarget.Length + 1) -replace "\\", "/"
                $sourceDocs += @{ Path = $rel; Type = "ADR" }
            }
        }
    }

    # Detect tech stack from config files
    $stackDetectors = @(
        @{ File = "*.csproj";        Tech = ".NET" },
        @{ File = "*.sln";           Tech = ".NET" },
        @{ File = "package.json";    Tech = "Node.js" },
        @{ File = "go.mod";          Tech = "Go" },
        @{ File = "Cargo.toml";      Tech = "Rust" },
        @{ File = "pyproject.toml";  Tech = "Python" },
        @{ File = "requirements.txt"; Tech = "Python" },
        @{ File = "Gemfile";         Tech = "Ruby" },
        @{ File = "pom.xml";         Tech = "Java/Maven" },
        @{ File = "build.gradle*";   Tech = "Java/Gradle" }
    )

    foreach ($detector in $stackDetectors) {
        $found = Get-ChildItem -Path $resolvedTarget -Filter $detector.File -Recurse -Depth 2 -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($found) {
            $techHints += $detector.Tech
        }
    }
    $techHints = $techHints | Select-Object -Unique
}

# --- Generate project.md ---
$projectContent = Get-Content -Path $projectTemplatePath -Raw
$projectContent = $projectContent -replace "# \[Project Name\]", "# $ProjectName"

# Inject discovered source documents
if ($sourceDocs.Count -gt 0) {
    $docTable = "| Path | Type | Notes |`n| --- | --- | --- |`n"
    foreach ($doc in $sourceDocs) {
        $docTable += "| ``$($doc.Path)`` | $($doc.Type) | |`n"
    }
    $projectContent = $projectContent -replace "\[Index of source docs\.\.\.\]", $docTable.TrimEnd("`n")
}

# Inject detected tech stack
if ($techHints.Count -gt 0) {
    $stackLine = ($techHints -join ", ")
    $projectContent = $projectContent -replace "\[Language / framework / major libs / infrastructure\]", $stackLine
}

$stateContent = Get-Content -Path $stateTemplatePath -Raw

Set-Content -Path $projectTargetPath -Value $projectContent
Set-Content -Path $stateTargetPath -Value $stateContent

& (Join-Path $scriptRoot "Sync-DevContext.ps1") -TargetPath $resolvedTarget
Invoke-GraphifyAutoInit -RepoPath $resolvedTarget

Write-Host "Initialized repo context in: $resolvedTarget"
Write-Host "- Created: .dev/project.md"
Write-Host "- Created: .dev/state.md"
Write-Host "- Ensured: docs/plans/"
Write-Host "- Generated: .github/copilot-instructions.md"
Write-Host "- Generated: GEMINI.md"
Write-Host "- Generated: CLAUDE.md"
Write-Host "- Generated: AGENTS.md"
Write-Host "- Generated: .agents/rules/gal.md"
Write-Host "- Next: review .dev/project.md, fill in summary fields, then run /gal status"

if ($sourceDocs.Count -gt 0) {
    Write-Host ""
    Write-Host "Adopt-existing: found $($sourceDocs.Count) source document(s)."
    Write-Host "Review .dev/project.md and fill in summaries from discovered docs."
}

if ($techHints.Count -gt 0) {
    Write-Host "Detected tech stack: $($techHints -join ', ')"
}