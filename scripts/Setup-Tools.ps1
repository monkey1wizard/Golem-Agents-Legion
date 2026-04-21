<#
.SYNOPSIS
    Interactive installer for optional GAL collaborative tools.

.DESCRIPTION
    Checks gstack, graphify, and OpenCLI, offers interactive installation for
    installable tools, installs only the selected tools using official upstream
    methods, and then verifies whether GAL can collaborate with each tool.

    OpenCLI Browser Bridge setup remains partially manual by design. This script
    can download the latest extension zip and prints the official install guide,
    GitHub links, and the full downloaded file path, but the user must still
    load the unpacked extension in Chrome or Chromium.

.PARAMETER Check
    Print statuses only without prompting or changing anything.

.PARAMETER Tool
    Limit the run to one or more tools. Accepts repeated values and comma-
    separated lists. Valid values: gstack, graphify, opencli.

.EXAMPLE
    .\scripts\Setup-Tools.ps1

.EXAMPLE
    .\scripts\Setup-Tools.ps1 -Check

.EXAMPLE
    .\scripts\Setup-Tools.ps1 -Tool gstack,opencli
#>
param(
    [switch]$Check,
    [string[]]$Tool
)

$ErrorActionPreference = 'Stop'

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptRoot

$supportedTools = @('gstack', 'graphify', 'opencli')
$gstackRoot = Join-Path $env:USERPROFILE '.claude\skills\gstack'
$gstackStateDir = Join-Path $env:USERPROFILE '.gstack'
$gstackConfigFile = Join-Path $gstackStateDir 'config.yaml'
$graphifyOutDir = Join-Path $repoRoot 'graphify-out'
$graphifyReportFile = Join-Path $graphifyOutDir 'GRAPH_REPORT.md'
$graphifyVersionFile = Join-Path $graphifyOutDir 'GAL_GRAPHIFY_VERSION.txt'
$openCliRepoUrl = 'https://github.com/jackwener/OpenCLI'
$openCliReleasesUrl = 'https://github.com/jackwener/OpenCLI/releases'
$openCliLatestApiUrl = 'https://api.github.com/repos/jackwener/OpenCLI/releases/latest'
$openCliDownloadsRoot = Join-Path $env:USERPROFILE 'Downloads'

function Write-Section([string]$Title) {
    Write-Host ''
    Write-Host ('=== {0} ===' -f $Title)
}

function Test-CommandAvailable([string]$Name) {
    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Get-PathEntries([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value)) { return @() }
    return @($Value -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
}

function Update-ProcessPath {
    $entries = New-Object System.Collections.Generic.List[string]
    foreach ($scope in @('Process', 'User', 'Machine')) {
        foreach ($entry in Get-PathEntries ([Environment]::GetEnvironmentVariable('Path', $scope))) {
            if (-not $entries.Contains($entry)) {
                [void]$entries.Add($entry)
            }
        }
    }

    [Environment]::SetEnvironmentVariable('Path', ($entries -join ';'), 'Process')
}

function Resolve-ToolSelection([string[]]$RawValues) {
    if ($null -eq $RawValues -or $RawValues.Count -eq 0) {
        return $supportedTools
    }

    $normalized = New-Object System.Collections.Generic.List[string]
    foreach ($value in $RawValues) {
        if ([string]::IsNullOrWhiteSpace($value)) { continue }
        foreach ($item in ($value -split ',')) {
            $trimmed = $item.Trim().ToLowerInvariant()
            if ([string]::IsNullOrWhiteSpace($trimmed)) { continue }
            if ($supportedTools -notcontains $trimmed) {
                throw "Unsupported tool '$trimmed'. Valid values: $($supportedTools -join ', ')."
            }
            if (-not $normalized.Contains($trimmed)) {
                [void]$normalized.Add($trimmed)
            }
        }
    }

    if ($normalized.Count -eq 0) {
        return $supportedTools
    }

    return @($normalized)
}

function Get-BashExecutable {
    $command = Get-Command bash -ErrorAction SilentlyContinue
    if ($null -ne $command) {
        return $command.Source
    }

    foreach ($candidate in @(
        'C:\Program Files\Git\bin\bash.exe',
        'C:\Program Files\Git\usr\bin\bash.exe'
    )) {
        if (Test-Path $candidate) {
            return $candidate
        }
    }

    return $null
}

function Get-PythonLauncher {
    foreach ($candidate in @(
        [pscustomobject]@{ Command = 'python'; Arguments = @() },
        [pscustomobject]@{ Command = 'py'; Arguments = @('-3') }
    )) {
        if (-not (Test-CommandAvailable $candidate.Command)) { continue }

        try {
            $version = & $candidate.Command @($candidate.Arguments + @('-c', 'import sys; print(f"{sys.version_info[0]}.{sys.version_info[1]}")')) 2>$null
            if ($LASTEXITCODE -eq 0 -and $version) {
                return $candidate
            }
        }
        catch {
        }
    }

    return $null
}

function Get-PythonVersionString([object]$Launcher) {
    if ($null -eq $Launcher) { return $null }

    try {
        $version = & $Launcher.Command @($Launcher.Arguments + @('-c', 'import sys; print(f"{sys.version_info[0]}.{sys.version_info[1]}")')) 2>$null
        if ($LASTEXITCODE -eq 0) {
            return ($version | Select-Object -First 1).Trim()
        }
    }
    catch {
    }

    return $null
}

function Test-PythonAtLeast310([object]$Launcher) {
    $versionString = Get-PythonVersionString $Launcher
    if ([string]::IsNullOrWhiteSpace($versionString)) {
        return $false
    }

    $parts = $versionString.Split('.')
    if ($parts.Count -lt 2) {
        return $false
    }

    $major = [int]$parts[0]
    $minor = [int]$parts[1]
    return ($major -gt 3) -or ($major -eq 3 -and $minor -ge 10)
}

function Test-PythonGraphifyModule([object]$Launcher) {
    if ($null -eq $Launcher) { return $false }

    try {
        & $Launcher.Command @($Launcher.Arguments + @('-m', 'graphify', '--version')) *> $null
        return $LASTEXITCODE -eq 0
    }
    catch {
        return $false
    }

    function Get-GraphifyVersion {
        if (-not (Test-CommandAvailable 'graphify')) {
            return $null
        }

        try {
            $version = & graphify --version 2>$null
            if ($LASTEXITCODE -eq 0 -and $version) {
                return ($version | Select-Object -First 1).Trim()
            }
        }
        catch {
        }

        return $null
    }
}

function Get-NodeMajorVersion {
    if (-not (Test-CommandAvailable 'node')) {
        return $null
    }

    try {
        $major = & node -p "process.versions.node.split('.')[0]" 2>$null
        if ($LASTEXITCODE -eq 0 -and $major) {
            return [int](($major | Select-Object -First 1).Trim())
        }
    }
    catch {
    }

    return $null
}

function Invoke-CapturedCommand([scriptblock]$ScriptBlock) {
    $output = & $ScriptBlock 2>&1 | Out-String
    $exitCode = if ($null -ne $LASTEXITCODE) { $LASTEXITCODE } else { 0 }
    return [pscustomobject]@{
        ExitCode = $exitCode
        Output   = $output.Trim()
    }
}

function New-ToolStatus {
    param(
        [string]$Name,
        [string]$Status,
        [string]$Reason,
        [bool]$CanInstall,
        [string]$InstallLabel,
        [string]$NextStep,
        [string]$ManualStep
    )

    return [pscustomobject]@{
        Name         = $Name
        Status       = $Status
        Reason       = $Reason
        CanInstall   = $CanInstall
        InstallLabel = $InstallLabel
        NextStep     = $NextStep
        ManualStep   = $ManualStep
    }
}

function Get-CollaborationLabel([string]$Status) {
    switch ($Status) {
        'ready' { return 'yes' }
        'available-but-needs-init' { return 'partially' }
        'available-but-not-ready' { return 'partially' }
        default { return 'no' }
    }
}

function Get-GstackStatus {
    $missing = New-Object System.Collections.Generic.List[string]
    if (-not (Test-CommandAvailable 'git')) {
        [void]$missing.Add('Git is missing')
    }
    if (-not (Test-CommandAvailable 'bun')) {
        [void]$missing.Add('Bun v1.0+ is missing')
    }
    if ($IsWindows -and -not (Test-CommandAvailable 'node')) {
        [void]$missing.Add('Node.js is required on Windows')
    }
    if ($IsWindows -and -not (Get-BashExecutable)) {
        [void]$missing.Add('Git Bash or bash is required to run ./setup on Windows')
    }

    if ($missing.Count -gt 0) {
        return New-ToolStatus -Name 'gstack' -Status 'unavailable' -Reason ($missing -join '; ') -CanInstall $false -InstallLabel 'Install gstack' -NextStep 'Install the missing prerequisites, then rerun this installer.' -ManualStep ''
    }

    if (-not (Test-Path $gstackRoot)) {
        return New-ToolStatus -Name 'gstack' -Status 'unavailable' -Reason ("Official checkout was not found at {0}" -f $gstackRoot) -CanInstall $true -InstallLabel 'Install gstack' -NextStep 'This installer can clone gstack and run ./setup.' -ManualStep ''
    }

    if (-not (Test-Path $gstackConfigFile)) {
        return New-ToolStatus -Name 'gstack' -Status 'available-but-needs-init' -Reason ("gstack checkout exists, but {0} is missing" -f $gstackConfigFile) -CanInstall $true -InstallLabel 'Finish gstack setup' -NextStep 'Run the official gstack setup flow so GAL can use machine-side collaboration lanes.' -ManualStep ''
    }

    return New-ToolStatus -Name 'gstack' -Status 'ready' -Reason 'Official checkout and ~/.gstack/config.yaml are present.' -CanInstall $false -InstallLabel 'Install gstack' -NextStep 'No action required.' -ManualStep ''
}

function Get-GraphifyStatus {
    $launcher = Get-PythonLauncher
    if ($null -eq $launcher) {
        return New-ToolStatus -Name 'graphify' -Status 'unavailable' -Reason 'Python 3.10+ is required but no supported Python launcher was found.' -CanInstall $false -InstallLabel 'Install graphify' -NextStep 'Install Python 3.10+ and rerun this installer.' -ManualStep ''
    }

    if (-not (Test-PythonAtLeast310 $launcher)) {
        return New-ToolStatus -Name 'graphify' -Status 'unavailable' -Reason ("Python {0} detected; graphify requires Python 3.10+." -f (Get-PythonVersionString $launcher)) -CanInstall $false -InstallLabel 'Install graphify' -NextStep 'Upgrade Python to 3.10+ and rerun this installer.' -ManualStep ''
    }

    $graphifyCli = Test-CommandAvailable 'graphify'
    $graphifyModule = Test-PythonGraphifyModule $launcher
    if (-not $graphifyCli -and -not $graphifyModule) {
        return New-ToolStatus -Name 'graphify' -Status 'unavailable' -Reason 'graphify CLI is not installed.' -CanInstall $true -InstallLabel 'Install graphify' -NextStep 'This installer can run the official graphify package install and graphify install flow.' -ManualStep ''
    }

    if (-not $graphifyCli -and $graphifyModule) {
        return New-ToolStatus -Name 'graphify' -Status 'available-but-needs-init' -Reason 'graphify is installed as a Python module, but the graphify command is not on PATH.' -CanInstall $false -InstallLabel 'Install graphify' -NextStep 'Open a new terminal or add your Python Scripts directory to PATH.' -ManualStep ''
    }

    if (-not (Test-Path $graphifyOutDir)) {
        return New-ToolStatus -Name 'graphify' -Status 'available-but-needs-init' -Reason 'graphify is installed, but graphify-out/ has not been generated for this repo yet.' -CanInstall $false -InstallLabel 'Install graphify' -NextStep 'Run /graphify . to generate graphify-out/ now. New repos initialized with /gal init will auto-run graphify when the CLI is already on PATH.' -ManualStep ''
    }

    if (-not (Test-Path $graphifyReportFile)) {
        return New-ToolStatus -Name 'graphify' -Status 'available-but-not-ready' -Reason 'graphify-out/ exists, but GRAPH_REPORT.md is missing.' -CanInstall $false -InstallLabel 'Install graphify' -NextStep 'Regenerate graphify outputs so graphify-out/GRAPH_REPORT.md exists.' -ManualStep ''
    }

    if (Test-Path $graphifyVersionFile) {
        $currentVersion = Get-GraphifyVersion
        $stampedVersion = (Get-Content -Path $graphifyVersionFile -ErrorAction SilentlyContinue | Select-Object -First 1)
        $reportInfo = Get-Item $graphifyReportFile
        $stampInfo = Get-Item $graphifyVersionFile

        if ($currentVersion -and $stampedVersion -and $stampedVersion.Trim() -ne $currentVersion -and $reportInfo.LastWriteTimeUtc -le $stampInfo.LastWriteTimeUtc) {
            return New-ToolStatus -Name 'graphify' -Status 'available-but-not-ready' -Reason ("graphify version changed since GAL last stamped this report (report: {0}, installed: {1})." -f $stampedVersion.Trim(), $currentVersion) -CanInstall $false -InstallLabel 'Install graphify' -NextStep 'Rerun /graphify . so graphify-out/ matches the installed graphify version.' -ManualStep ''
        }
    }

    return New-ToolStatus -Name 'graphify' -Status 'ready' -Reason 'graphify CLI and graphify-out/GRAPH_REPORT.md are both present.' -CanInstall $false -InstallLabel 'Install graphify' -NextStep 'No action required.' -ManualStep ''
}

function Get-OpenCliStatus {
    $openCliInstalled = Test-CommandAvailable 'opencli'
    if (-not $openCliInstalled) {
        $nodeMajor = Get-NodeMajorVersion
        if ($null -eq $nodeMajor) {
            return New-ToolStatus -Name 'opencli' -Status 'unavailable' -Reason 'Node.js 21+ and npm are required for the official OpenCLI install path.' -CanInstall $false -InstallLabel 'Install OpenCLI' -NextStep 'Install Node.js 21+ and npm, then rerun this installer.' -ManualStep ''
        }
        if ($nodeMajor -lt 21) {
            return New-ToolStatus -Name 'opencli' -Status 'unavailable' -Reason ("Node.js {0} detected; OpenCLI requires Node.js 21+." -f $nodeMajor) -CanInstall $false -InstallLabel 'Install OpenCLI' -NextStep 'Upgrade Node.js to 21+ and rerun this installer.' -ManualStep ''
        }
        if (-not (Test-CommandAvailable 'npm')) {
            return New-ToolStatus -Name 'opencli' -Status 'unavailable' -Reason 'npm is required for the official OpenCLI install path.' -CanInstall $false -InstallLabel 'Install OpenCLI' -NextStep 'Install npm and rerun this installer.' -ManualStep ''
        }

        return New-ToolStatus -Name 'opencli' -Status 'unavailable' -Reason 'opencli CLI is not installed.' -CanInstall $true -InstallLabel 'Install OpenCLI' -NextStep 'This installer can run npm install -g @jackwener/opencli.' -ManualStep ''
    }

    $doctor = Invoke-CapturedCommand { & opencli doctor }
    if ($doctor.ExitCode -eq 0) {
        return New-ToolStatus -Name 'opencli' -Status 'ready' -Reason 'opencli doctor succeeded.' -CanInstall $false -InstallLabel 'Install OpenCLI' -NextStep 'No action required.' -ManualStep ''
    }

    $reason = 'OpenCLI CLI exists, but opencli doctor reported incomplete browser bridge or local session wiring.'
    if (-not [string]::IsNullOrWhiteSpace($doctor.Output)) {
        $reason = '{0} Last doctor output: {1}' -f $reason, ($doctor.Output -replace '\s+', ' ').Trim()
    }

    $manualStep = 'Complete the Browser Bridge installation in Chrome or Chromium, then rerun opencli doctor.'
    return New-ToolStatus -Name 'opencli' -Status 'available-but-needs-init' -Reason $reason -CanInstall $true -InstallLabel 'Install OpenCLI / download Browser Bridge' -NextStep $manualStep -ManualStep $manualStep
}

function Get-ToolStatus([string]$Name) {
    switch ($Name) {
        'gstack' { return Get-GstackStatus }
        'graphify' { return Get-GraphifyStatus }
        'opencli' { return Get-OpenCliStatus }
        default { throw "Unsupported tool '$Name'." }
    }
}

function Write-StatusReport([System.Collections.Generic.List[object]]$Statuses) {
    Write-Section 'Collaborative Tool Status'
    foreach ($status in $Statuses) {
        Write-Host ('  - {0}: {1}' -f $status.Name, $status.Status)
        Write-Host ('    {0}' -f $status.Reason)
    }
}

function ConvertFrom-MultiSelectAnswer([string]$Answer, [int]$MaxIndex) {
    if ([string]::IsNullOrWhiteSpace($Answer)) {
        return @()
    }

    $selected = New-Object System.Collections.Generic.List[int]
    foreach ($segment in ($Answer -split ',')) {
        $trimmed = $segment.Trim()
        if ([string]::IsNullOrWhiteSpace($trimmed)) { continue }
        $number = 0
        if (-not [int]::TryParse($trimmed, [ref]$number)) {
            throw "Invalid selection '$trimmed'."
        }
        if ($number -lt 1 -or $number -gt $MaxIndex) {
            throw "Selection '$trimmed' is out of range."
        }
        if (-not $selected.Contains($number)) {
            [void]$selected.Add($number)
        }
    }

    return @($selected)
}

function Read-InstallSelection([System.Collections.Generic.List[object]]$Candidates) {
    if ($Candidates.Count -eq 0) {
        Write-Host '  [OK] No collaborative tools need installation or installer-assisted setup.'
        return @()
    }

    if ($Candidates.Count -eq 1) {
        $candidate = $Candidates[0]
        $answer = Read-Host ("  [PROMPT] {0} is not currently ready. Install {0} now? [Y/n]" -f $candidate.Name)
        if ($answer -match '^(n|no)$') {
            Write-Host ('  [SKIP] {0} installation skipped by user.' -f $candidate.Name)
            return @()
        }

        return @($candidate.Name)
    }

    Write-Host '  [PROMPT] The following tools are missing or still need install-time setup. Enter one or more numbers separated by commas, or press Enter to skip all.'
    for ($index = 0; $index -lt $Candidates.Count; $index++) {
        $item = $Candidates[$index]
        Write-Host ('    {0}. {1} - {2}' -f ($index + 1), $item.Name, $item.Reason)
    }

    while ($true) {
        $answer = Read-Host '  [PROMPT] Enter selection numbers (for example: 1,3)'
        try {
            $selectedIndices = ConvertFrom-MultiSelectAnswer -Answer $answer -MaxIndex $Candidates.Count
            if ($selectedIndices.Count -eq 0) {
                Write-Host '  [SKIP] No collaborative tools selected for installation.'
                return @()
            }

            return @($selectedIndices | ForEach-Object { $Candidates[$_ - 1].Name })
        }
        catch {
            Write-Host ('  [WARN] {0}' -f $_.Exception.Message) -ForegroundColor Yellow
        }
    }
}

function Invoke-BashCommand([string]$CommandText) {
    $bash = Get-BashExecutable
    if ([string]::IsNullOrWhiteSpace($bash)) {
        throw 'bash was not found. Install Git Bash or another bash runtime and rerun this installer.'
    }

    & $bash -lc $CommandText
    if ($LASTEXITCODE -ne 0) {
        throw ("bash command failed with exit code {0}." -f $LASTEXITCODE)
    }
}

function Install-Gstack {
    Write-Section 'Install gstack'
    Write-Host '  [INFO] Running the official gstack clone + setup flow.'
    $command = @(
        'set -euo pipefail',
        'mkdir -p "$HOME/.claude/skills"',
        'if [ ! -d "$HOME/.claude/skills/gstack/.git" ]; then git clone --single-branch --depth 1 https://github.com/garrytan/gstack.git "$HOME/.claude/skills/gstack"; fi',
        'cd "$HOME/.claude/skills/gstack"',
        './setup'
    ) -join '; '

    Invoke-BashCommand $command
    Write-Host '  [OK] gstack install flow completed.'
}

function Get-GraphifyRunner([object]$Launcher) {
    if (Test-CommandAvailable 'graphify') {
        return [pscustomobject]@{ Command = 'graphify'; Arguments = @() }
    }

    return [pscustomobject]@{ Command = $Launcher.Command; Arguments = @($Launcher.Arguments + @('-m', 'graphify')) }
}

function Install-Graphify {
    Write-Section 'Install graphify'
    $launcher = Get-PythonLauncher
    if ($null -eq $launcher -or -not (Test-PythonAtLeast310 $launcher)) {
        throw 'Python 3.10+ is required before graphify can be installed.'
    }

    Write-Host '  [INFO] Installing the official graphifyy package.'
    & $launcher.Command @($launcher.Arguments + @('-m', 'pip', 'install', 'graphifyy'))
    if ($LASTEXITCODE -ne 0) {
        throw 'python -m pip install graphifyy failed.'
    }

    Update-ProcessPath
    $runner = Get-GraphifyRunner $launcher
    Write-Host '  [INFO] Running the official graphify install command.'
    & $runner.Command @($runner.Arguments + @('install'))
    if ($LASTEXITCODE -ne 0) {
        throw 'graphify install failed.'
    }

    Write-Host '  [OK] graphify install flow completed.'
}

function Get-OpenCliLatestReleaseAsset {
    try {
        return Invoke-RestMethod -Uri $openCliLatestApiUrl -Headers @{ 'User-Agent' = 'GAL-CollaborativeTools-Installer' }
    }
    catch {
        throw ("Failed to query {0}: {1}" -f $openCliLatestApiUrl, $_.Exception.Message)
    }
}

function Save-OpenCliExtensionDownload {
    Write-Host '  [INFO] Resolving the latest OpenCLI Browser Bridge release asset.'
    $release = Get-OpenCliLatestReleaseAsset
    $asset = @($release.assets | Where-Object { $_.name -match '^opencli-extension-.*\.zip$' } | Select-Object -First 1)
    if ($asset.Count -eq 0) {
        throw 'Could not find an opencli-extension-*.zip asset in the latest OpenCLI release.'
    }

    if (-not (Test-Path $openCliDownloadsRoot)) {
        New-Item -ItemType Directory -Path $openCliDownloadsRoot -Force | Out-Null
    }

    $targetPath = Join-Path $openCliDownloadsRoot $asset[0].name
    if (Test-Path $targetPath) {
        Write-Host ('  [SKIP] OpenCLI Browser Bridge zip already exists: {0}' -f $targetPath)
        return $targetPath
    }

    Write-Host ('  [INFO] Downloading {0}' -f $asset[0].browser_download_url)
    Invoke-WebRequest -Uri $asset[0].browser_download_url -OutFile $targetPath
    Write-Host ('  [OK] Downloaded OpenCLI Browser Bridge zip to {0}' -f $targetPath)
    return $targetPath
}

function Write-OpenCliInstallGuide([string]$DownloadedPath) {
    Write-Host ''
    Write-Host '  [INFO] OpenCLI Browser Bridge install guide (official upstream):'
    Write-Host '    1. Download the latest opencli-extension-v{version}.zip from the GitHub Releases page.'
    Write-Host '    2. Unzip it, open chrome://extensions, and enable Developer mode.'
    Write-Host '    3. Click Load unpacked and select the unzipped folder.'
    Write-Host ('  [INFO] Downloaded file path: {0}' -f $DownloadedPath)
    Write-Host ('  [INFO] GitHub repo: {0}' -f $openCliRepoUrl)
    Write-Host ('  [INFO] Releases page: {0}' -f $openCliReleasesUrl)
    Write-Host '  [INFO] After completing those steps, rerun `opencli doctor`.'
}

function Install-OpenCli {
    Write-Section 'Install OpenCLI'
    $nodeMajor = Get-NodeMajorVersion
    if ($null -eq $nodeMajor -or $nodeMajor -lt 21) {
        throw 'Node.js 21+ is required before OpenCLI can be installed.'
    }
    if (-not (Test-CommandAvailable 'npm')) {
        throw 'npm is required before OpenCLI can be installed.'
    }

    if (-not (Test-CommandAvailable 'opencli')) {
        Write-Host '  [INFO] Running the official OpenCLI npm install command.'
        & npm install -g @jackwener/opencli
        if ($LASTEXITCODE -ne 0) {
            throw 'npm install -g @jackwener/opencli failed.'
        }
        Update-ProcessPath
    }
    else {
        Write-Host '  [OK] opencli CLI already exists; skipping npm install.'
    }

    if (-not (Test-CommandAvailable 'opencli')) {
        throw 'OpenCLI install completed, but the opencli command is still not available in this shell.'
    }

    $downloadedPath = Save-OpenCliExtensionDownload
    Write-OpenCliInstallGuide -DownloadedPath $downloadedPath
    return $downloadedPath
}

function Write-FinalSummary([System.Collections.Generic.List[object]]$Summaries) {
    Write-Section 'Final Summary'
    foreach ($item in $Summaries) {
        Write-Host ('  - {0}' -f $item.Name)
        Write-Host ('    before: {0}' -f $item.Before)
        Write-Host ('    action: {0}' -f $item.Action)
        Write-Host ('    after:  {0}' -f $item.After.Status)
        Write-Host ('    GAL collaboration: {0}' -f (Get-CollaborationLabel $item.After.Status))
        Write-Host ('    reason: {0}' -f $item.After.Reason)
        if (-not [string]::IsNullOrWhiteSpace($item.After.NextStep) -and $item.After.Status -ne 'ready') {
            Write-Host ('    next:   {0}' -f $item.After.NextStep)
        }
        if ($item.Name -eq 'opencli' -and -not [string]::IsNullOrWhiteSpace($item.DownloadedPath)) {
            Write-Host ('    downloaded extension zip: {0}' -f $item.DownloadedPath)
            Write-Host ('    github: {0}' -f $openCliRepoUrl)
        }
    }
}

$selectedTools = Resolve-ToolSelection $Tool
$statuses = New-Object 'System.Collections.Generic.List[object]'
foreach ($toolName in $selectedTools) {
    [void]$statuses.Add((Get-ToolStatus $toolName))
}

Write-StatusReport $statuses

if ($Check) {
    return
}

$candidates = New-Object 'System.Collections.Generic.List[object]'
foreach ($status in $statuses) {
    if ($status.CanInstall -and @('unavailable', 'available-but-needs-init') -contains $status.Status) {
        [void]$candidates.Add($status)
    }
}

$selectedInstallNames = Read-InstallSelection $candidates
$selectedLookup = @{}
foreach ($name in $selectedInstallNames) {
    $selectedLookup[$name] = $true
}

$summaries = New-Object 'System.Collections.Generic.List[object]'

foreach ($toolName in $selectedTools) {
    $before = Get-ToolStatus $toolName
    $action = 'no-op'
    $downloadedPath = $null

    if ($selectedLookup.ContainsKey($toolName)) {
        $latest = Get-ToolStatus $toolName
        if ($latest.Status -eq 'ready') {
            $action = 'skipped - already ready before install step'
        }
        elseif (-not $latest.CanInstall) {
            $action = 'skipped - install not applicable for this status'
        }
        else {
            try {
                switch ($toolName) {
                    'gstack' {
                        Install-Gstack
                        $action = 'installed via official clone + setup'
                    }
                    'graphify' {
                        Install-Graphify
                        $action = 'installed via official graphifyy + graphify install'
                    }
                    'opencli' {
                        $downloadedPath = Install-OpenCli
                        $action = 'installed CLI and downloaded Browser Bridge zip'
                    }
                }
            }
            catch {
                $action = 'install failed'
                Write-Host ('  [ERROR] {0} install failed: {1}' -f $toolName, $_.Exception.Message) -ForegroundColor Red
            }
        }
    }
    elseif ($candidates | Where-Object { $_.Name -eq $toolName }) {
        $action = 'skipped by user'
    }

    $after = Get-ToolStatus $toolName
    [void]$summaries.Add([pscustomobject]@{
        Name           = $toolName
        Before         = $before.Status
        Action         = $action
        After          = $after
        DownloadedPath = $downloadedPath
    })
}

Write-FinalSummary $summaries