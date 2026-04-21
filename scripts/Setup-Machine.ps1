<#
.SYNOPSIS
    Creates symlinks from golem-agents-legion repo to Copilot, Gemini, Codex, and Claude runtime directories.

.DESCRIPTION
        Links:
            - agent/*.agent.md  → ~/.copilot/agents/*.agent.md
            - skills/*/         → ~/.copilot/skills/*/
            - skills/*/         → ~/.agents/skills/*/ (shared reusable skills for Gemini CLI + Codex CLI)
            - skills/*/         → ~/.claude/skills/*/
            - commands/*/       → ~/.copilot/skills/*/ + ~/.codex/skills/*/ (baked command skills)
            - commands/*/       → ~/.gemini/commands/*.toml + ~/.claude/commands/*.md
            - <repo root>       → ~/.copilot/gal/ + ~/.gemini/gal/ (GAL_ROOT dir symlinks)
    - Generates commands/*/SKILL.md from SKILL.template.md (baked absolute paths) plus optional gitignored SKILL.local.md overlays
    - Generates ~/.gemini/commands/*.toml so Gemini CLI can expose GAL commands natively
    - Generates ~/.claude/commands/*.md so Claude Code can expose GAL commands natively
    - Generates ~/.gemini/gal-context.md (@file skill imports, paths reference .agents/skills)
    - Persists machine-local runtime selection in ~/.gal/install-state.json
    - Merges MCP server config from mcp-servers.example.json + mcp-servers.local.json into VS Code, Gemini, and Codex user config files

    On Windows, requires Developer Mode enabled or admin privileges for symlinks.
    Falls back to directory junctions for skill folders if symlinks fail.

.PARAMETER Uninstall
    Remove all symlinks created by this script instead of creating them.

.PARAMETER Replace
    When a real (non-link) directory/file already exists at the target, rename it
    to *.bak and create the symlink. Without this flag, existing items are skipped.

.PARAMETER DryRun
    Show what would be done without making changes.

.PARAMETER Reconfigure
    Prompt again for selected runtimes and primary runtime, then update the install-state file.

.EXAMPLE
    .\scripts\Setup-Machine.ps1
    .\scripts\Setup-Machine.ps1 -Replace
    .\scripts\Setup-Machine.ps1 -DryRun
    .\scripts\Setup-Machine.ps1 -Reconfigure
    .\scripts\Setup-Machine.ps1 -Uninstall
#>
#Requires -Version 5.1
param(
    [switch]$Uninstall,
    [switch]$Replace,
    [switch]$DryRun,
    [switch]$Reconfigure
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptRoot
$utf8NoBom = [System.Text.UTF8Encoding]::new($false)

$galStateRoot = Join-Path $env:USERPROFILE ".gal"
$installStateFile = Join-Path $galStateRoot "install-state.json"

$copilotRoot = Join-Path $env:USERPROFILE ".copilot"
$agentsTarget = Join-Path $copilotRoot "agents"
$skillsTarget = Join-Path $copilotRoot "skills"

$sharedAgentsRoot = Join-Path $env:USERPROFILE ".agents"
$sharedSkillsTarget = Join-Path $sharedAgentsRoot "skills"

$geminiRoot = Join-Path $env:USERPROFILE ".gemini"
$geminiSkillsTarget = Join-Path $geminiRoot "skills"
$geminiCommandsTarget = Join-Path $geminiRoot "commands"
$geminiContextFile = Join-Path $geminiRoot "gal-context.md"
$geminiSettingsFile = Join-Path $geminiRoot "settings.json"
$vscodeSettingsFile = Join-Path $env:APPDATA "Code\User\settings.json"
$vscodeMcpFile = Join-Path $env:APPDATA "Code\User\mcp.json"

$codexRoot = Join-Path $env:USERPROFILE ".codex"
$codexSkillsTarget = Join-Path $codexRoot "skills"
$codexConfigFile = Join-Path $env:USERPROFILE ".codex\config.toml"

$claudeRoot = Join-Path $env:USERPROFILE ".claude"
$claudeSkillsTarget = Join-Path $claudeRoot "skills"
$claudeCommandsTarget = Join-Path $claudeRoot "commands"

$mcpManifestExampleFile = Join-Path $repoRoot "mcp-servers.example.json"
$mcpManifestLocalFile = Join-Path $repoRoot "mcp-servers.local.json"

$galSource       = Join-Path $repoRoot "commands\gal"
$galRootCopilot  = Join-Path $copilotRoot "gal"
$galRootGemini   = Join-Path $geminiRoot "gal"
$galSkillCopilot = Join-Path $skillsTarget "gal"
$galSkillCodex   = Join-Path $codexSkillsTarget "gal"
$skillTemplate   = Join-Path $galSource "SKILL.template.md"
$commandsSourceDir = Join-Path $repoRoot "commands"
$commandAliasNames = Get-ChildItem $commandsSourceDir -Directory |
    Where-Object {
        $_.Name -ne 'gal' -and (
            (Test-Path (Join-Path $_.FullName 'SKILL.template.md')) -or
            (Test-Path (Join-Path $_.FullName 'SKILL.md'))
        )
    } |
    Select-Object -ExpandProperty Name

$commandSkillDirs = @(
    [pscustomobject]@{
        Name          = 'gal'
        Source        = $galSource
        Template      = $skillTemplate
        CopilotTarget = $galSkillCopilot
        CodexTarget   = $galSkillCodex
    }
) + ($commandAliasNames | ForEach-Object {
    $source = Join-Path $repoRoot ("commands\{0}" -f $_)
    [pscustomobject]@{
        Name          = $_
        Source        = $source
        Template      = Join-Path $source "SKILL.template.md"
        CopilotTarget = Join-Path $skillsTarget $_
        CodexTarget   = Join-Path $codexSkillsTarget $_
    }
})
$activeCommandSkillNames = $commandSkillDirs | ForEach-Object { $_.Name }
$galManagedFileHeader = '# Generated by GAL Setup-Machine. Do not edit manually.'
$runtimeCatalog = @(
    [pscustomobject]@{ Key = 'copilot'; Label = 'VS Code Copilot'; Description = 'agents, skills, GAL commands, VS Code settings bridge, VS Code MCP bridge' },
    [pscustomobject]@{ Key = 'gemini'; Label = 'Gemini CLI'; Description = 'native command files, GAL context, shared skills, Gemini MCP bridge' },
    [pscustomobject]@{ Key = 'codex'; Label = 'Codex CLI'; Description = 'installed GAL command skills, shared skills, Codex MCP bridge' },
    [pscustomobject]@{ Key = 'claude'; Label = 'Claude Code'; Description = 'Claude skills, native command files, repo-local CLAUDE.md adapter' }
)

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

function Test-GalManagedFile([string]$Path) {
    if (-not (Test-Path $Path)) { return $false }
    try {
        $firstLine = Get-Content $Path -Encoding UTF8 -TotalCount 1 -ErrorAction Stop
        return $firstLine -eq $galManagedFileHeader
    }
    catch {
        return $false
    }
}

function Get-BakedCommandSkillContent([string]$TemplatePath, [string]$LocalOverridePath) {
    $baked = (Get-Content $TemplatePath -Raw) -replace [regex]::Escape('{{GAL_ROOT}}'), $repoRoot

    if (-not (Test-Path $LocalOverridePath)) {
        return $baked
    }

    $localOverride = Get-Content $LocalOverridePath -Raw
    if ([string]::IsNullOrWhiteSpace($localOverride)) {
        return $baked
    }

    return @(
        $baked.TrimEnd("`r", "`n")
        ''
        '<!-- GAL LOCAL OVERRIDE START -->'
        '<!-- Source: SKILL.local.md (gitignored machine-local overlay) -->'
        $localOverride.Trim("`r", "`n")
        '<!-- GAL LOCAL OVERRIDE END -->'
        ''
    ) -join "`n"
}

function Test-GalRepoLink([string]$Path, [string]$TargetFragment = $repoRoot) {
    if (-not (Test-Path $Path)) { return $false }

    $item = Get-Item $Path -Force -ErrorAction SilentlyContinue
    if ($null -eq $item) { return $false }

    $isReparsePoint = ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0
    if (-not $isReparsePoint) { return $false }

    foreach ($target in @($item.Target)) {
        if ([string]::IsNullOrWhiteSpace([string]$target)) { continue }
        if ([string]$target -like "*$TargetFragment*") {
            return $true
        }
    }

    return $false
}

function Test-GalCommandLink([string]$Path) {
    if (-not (Test-Path $Path)) { return $false }

    $item = Get-Item $Path -Force -ErrorAction SilentlyContinue
    if ($null -eq $item) { return $false }

    $isReparsePoint = ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0
    if (-not $isReparsePoint) { return $false }

    $targets = @($item.Target)
    if ($targets.Count -eq 0) { return $false }

    foreach ($target in $targets) {
        if ([string]::IsNullOrWhiteSpace([string]$target)) { continue }
        if ([string]$target -like "*$repoRoot*commands*") {
            return $true
        }
    }

    return $false
}

function Get-SkillFrontmatterDescription([string]$RawContent) {
    if ($RawContent -match '(?ms)^---\r?\n(?<frontmatter>.*?)\r?\n---\r?\n?') {
        $frontmatter = $matches['frontmatter']
        if ($frontmatter -match '(?m)^description:\s*"(?<description>.*)"\s*$') {
            return $matches['description'].Trim()
        }
        if ($frontmatter -match '(?m)^description:\s*(?<description>.+?)\s*$') {
            return $matches['description'].Trim().Trim('"')
        }
    }

    return $null
}

function Get-SkillMarkdownBody([string]$RawContent) {
    if ($RawContent -match '(?ms)^---\r?\n.*?\r?\n---\r?\n?(?<body>.*)$') {
        return $matches['body']
    }

    return $RawContent
}

function Convert-ToTomlMultilineLiteralString([string]$Text) {
    $normalized = $Text -replace "`r`n", "`n" -replace "`r", "`n"
    return "'''" + "`n" + $normalized.TrimEnd() + "`n" + "'''"
}

function New-GeminiCommandFileContent([string]$SkillPath) {
    $rawContent = Get-Content $SkillPath -Raw -Encoding UTF8
    $description = Get-SkillFrontmatterDescription $rawContent
    if ([string]::IsNullOrWhiteSpace($description)) {
        $description = "GAL command"
    }

    $body = (Get-SkillMarkdownBody $rawContent).Trim()
    $prompt = @(
        'User command arguments, if any: {{args}}'
        ''
        $body
    ) -join "`n"

    $escapedDescription = $description.Replace('\\', '\\\\').Replace('"', '\\"')

    return @(
        $galManagedFileHeader
        ('description = "{0}"' -f $escapedDescription)
        ("prompt = {0}" -f (Convert-ToTomlMultilineLiteralString $prompt))
        ''
    ) -join "`n"
}

function New-ClaudeCommandFileContent([string]$SkillPath, [string]$CommandName) {
    $rawContent = Get-Content $SkillPath -Raw -Encoding UTF8
    $description = Get-SkillFrontmatterDescription $rawContent
    if ([string]::IsNullOrWhiteSpace($description)) {
        $description = "GAL command"
    }

    $body = (Get-SkillMarkdownBody $rawContent).Trim()
    $indentedDescription = ($description -replace "`r`n", "`n" -replace "`r", "`n") -split "`n" | ForEach-Object { '  ' + $_ }

    return @(
        $galManagedFileHeader
        '---'
        'description: |'
        ($indentedDescription -join "`n")
        '---'
        ''
        ('# {0}' -f $CommandName)
        ''
        'User command arguments, if any: {{args}}'
        ''
        $body
        ''
    ) -join "`n"
}

function Test-CommandAvailable([string]$Name) {
    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Get-PathEntries([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value)) { return @() }
    return @($Value -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
}

function Refresh-ProcessPath {
    $entries = New-Object System.Collections.Generic.List[string]
    $seen = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($scope in @('Process', 'User', 'Machine')) {
        foreach ($entry in Get-PathEntries ([Environment]::GetEnvironmentVariable('Path', $scope))) {
            if ($seen.Add($entry)) {
                [void]$entries.Add($entry)
            }
        }
    }

    [Environment]::SetEnvironmentVariable('Path', ($entries -join ';'), 'Process')
}

function Test-WingetPackageInstalled([string]$PackageId) {
    if (-not (Test-CommandAvailable 'winget')) { return $false }

    $output = & winget list --id $PackageId -e --accept-source-agreements 2>$null | Out-String
    if ($LASTEXITCODE -ne 0) { return $false }

    return $output -match [regex]::Escape($PackageId)
}

function Ensure-Ripgrep {
    if ($Uninstall) { return }

    Write-Host ""
    Write-Host "=== ripgrep (rg) ==="

    if (Test-CommandAvailable 'rg') {
        Write-Host "  [OK] rg available"
        return
    }

    Refresh-ProcessPath
    if (Test-CommandAvailable 'rg') {
        Write-Host "  [OK] rg available after PATH refresh"
        return
    }

    $packageId = 'BurntSushi.ripgrep.MSVC'
    if (Test-WingetPackageInstalled $packageId) {
        Write-Host "  [WARN] ripgrep appears installed, but the current shell still cannot resolve rg. Open a new terminal and rerun your command." -ForegroundColor Yellow
        return
    }

    if (-not (Test-CommandAvailable 'winget')) {
        Write-Host "  [WARN] rg not found and winget is unavailable. Install ripgrep manually, then reopen the terminal." -ForegroundColor Yellow
        return
    }

    if ($DryRun) {
        Write-Host "  [DRY RUN] rg missing. Would ask to install $packageId via winget."
        return
    }

    $answer = Read-Host "  [PROMPT] ripgrep (rg) was not found. Install it now via winget? [Y/n]"
    if ($answer -match '^(n|no)$') {
        Write-Host "  [SKIP] ripgrep installation skipped"
        return
    }

    try {
        & winget install --id $packageId -e --accept-package-agreements --accept-source-agreements
    }
    catch {
        Write-Host "  [WARN] ripgrep installation failed: $_" -ForegroundColor Yellow
        return
    }

    Refresh-ProcessPath
    if (Test-CommandAvailable 'rg') {
        Write-Host "  [OK] ripgrep installed and available"
    }
    else {
        Write-Host "  [WARN] ripgrep was installed, but this shell still cannot resolve rg. Open a new terminal and rerun your command." -ForegroundColor Yellow
    }
}

function ConvertTo-OrderedMap([object]$InputObject) {
    if ($null -eq $InputObject) { return $null }

    if ($InputObject -is [System.Collections.IDictionary]) {
        $map = [ordered]@{}
        foreach ($key in $InputObject.Keys) {
            $map[$key] = ConvertTo-OrderedMap $InputObject[$key]
        }
        return $map
    }

    if ($InputObject -is [System.Collections.IEnumerable] -and -not ($InputObject -is [string])) {
        $items = [System.Collections.Generic.List[object]]::new()
        foreach ($item in $InputObject) {
            $items.Add((ConvertTo-OrderedMap $item))
        }
        return @($items)
    }

    if ($InputObject.PSObject -and $InputObject -isnot [string] -and $InputObject -isnot [ValueType]) {
        $map = [ordered]@{}
        foreach ($property in $InputObject.PSObject.Properties) {
            $map[$property.Name] = ConvertTo-OrderedMap $property.Value
        }
        return $map
    }

    return $InputObject
}

function Merge-OrderedMap([System.Collections.IDictionary]$Base, [System.Collections.IDictionary]$Overlay) {
    foreach ($key in $Overlay.Keys) {
        if (
            $Base.Contains($key) -and
            $Base[$key] -is [System.Collections.IDictionary] -and
            $Overlay[$key] -is [System.Collections.IDictionary]
        ) {
            $Base[$key] = Merge-OrderedMap $Base[$key] $Overlay[$key]
        }
        else {
            $Base[$key] = $Overlay[$key]
        }
    }

    return $Base
}

function Read-JsonOrderedMap([string]$Path) {
    if (-not (Test-Path $Path)) {
        return [ordered]@{}
    }

    $rawJson = Get-Content $Path -Raw -Encoding UTF8
    if ([string]::IsNullOrWhiteSpace($rawJson)) {
        return [ordered]@{}
    }

    try {
        return ConvertTo-OrderedMap ($rawJson | ConvertFrom-Json)
    }
    catch {
        Write-Host "  [WARN] Could not parse JSON file: $Path" -ForegroundColor Yellow
        return $null
    }
}

function Write-JsonOrderedMap([string]$Path, [System.Collections.IDictionary]$Data) {
    $directory = Split-Path $Path -Parent
    if (-not (Test-Path $directory)) {
        New-Item -ItemType Directory -Path $directory -Force | Out-Null
    }

    [System.IO.File]::WriteAllText($Path, ($Data | ConvertTo-Json -Depth 20), $utf8NoBom)
}

function Read-KeyValueEnvFile([string]$Path) {
    $values = [ordered]@{}
    if (-not (Test-Path $Path)) {
        return $values
    }

    foreach ($line in Get-Content $Path -Encoding UTF8) {
        if ($line -match '^\s*#' -or $line -notmatch '=') { continue }

        $parts = $line.Split('=', 2)
        $key = $parts[0].Trim()
        $value = $parts[1].Trim()
        if ([string]::IsNullOrWhiteSpace($key) -or [string]::IsNullOrWhiteSpace($value)) { continue }
        $values[$key] = $value
    }

    return $values
}

function Get-ConfiguredValue([System.Collections.IDictionary]$Values, [string]$Name) {
    if ($Values.Contains($Name) -and -not [string]::IsNullOrWhiteSpace([string]$Values[$Name])) {
        return [string]$Values[$Name]
    }

    $userValue = [Environment]::GetEnvironmentVariable($Name, 'User')
    if (-not [string]::IsNullOrWhiteSpace($userValue)) { return $userValue }

    $processValue = [Environment]::GetEnvironmentVariable($Name, 'Process')
    if (-not [string]::IsNullOrWhiteSpace($processValue)) { return $processValue }

    return $null
}

function Split-ConfigList([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value)) { return @() }
    return @($Value -split '\s*,\s*' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
}

function Get-McpVariableMap([System.Collections.IDictionary]$LocalEnvValues) {
    $values = [ordered]@{}
    foreach ($key in $LocalEnvValues.Keys) {
        $values[$key] = $LocalEnvValues[$key]
    }

    if (-not $values.Contains('MCP_MEMORY_FILE_PATH')) {
        $values['MCP_MEMORY_FILE_PATH'] = Join-Path $env:USERPROFILE 'mcp-memory.json'
    }

    if (-not $values.Contains('OBSIDIAN_VERIFY_SSL')) {
        $values['OBSIDIAN_VERIFY_SSL'] = 'false'
    }

    if (-not $values.Contains('OBSIDIAN_ENABLE_CACHE')) {
        $values['OBSIDIAN_ENABLE_CACHE'] = 'true'
    }

    if (-not $values.Contains('MCP_FILESYSTEM_PATHS')) {
        $paths = @((Split-Path $repoRoot -Parent))
        $obsidianVault = Get-ConfiguredValue $values 'OBSIDIAN_VAULT'
        if (-not [string]::IsNullOrWhiteSpace($obsidianVault)) {
            $paths += $obsidianVault
        }
        $values['MCP_FILESYSTEM_PATHS'] = (($paths | Select-Object -Unique) -join ',')
    }

    return $values
}

function Resolve-McpString([string]$Value, [System.Collections.IDictionary]$Values) {
    if ($Value -match '^\$\{([A-Z0-9_]+)\}$') {
        $resolved = Get-ConfiguredValue $Values $Matches[1]
        if ($null -ne $resolved) { return $resolved }
    }

    return [regex]::Replace($Value, '\$\{([A-Z0-9_]+)\}', {
        param($match)
        $resolved = Get-ConfiguredValue $Values $match.Groups[1].Value
        if ($null -ne $resolved) { return $resolved }
        return $match.Value
    })
}

function Resolve-McpNode([object]$Node, [System.Collections.IDictionary]$Values, [switch]$ExpandArrayPlaceholder) {
    if ($null -eq $Node) { return $null }

    if ($Node -is [string]) {
        if ($ExpandArrayPlaceholder -and $Node -match '^\$\{([A-Z0-9_]+)\[\]\}$') {
            return @(Split-ConfigList (Get-ConfiguredValue $Values $Matches[1]))
        }
        return Resolve-McpString $Node $Values
    }

    if ($Node -is [System.Collections.IDictionary]) {
        $map = [ordered]@{}
        foreach ($key in $Node.Keys) {
            $map[$key] = Resolve-McpNode $Node[$key] $Values
        }
        return $map
    }

    if ($Node -is [System.Collections.IEnumerable] -and -not ($Node -is [string])) {
        $items = [System.Collections.Generic.List[object]]::new()
        foreach ($item in $Node) {
            $resolvedItem = Resolve-McpNode $item $Values -ExpandArrayPlaceholder:$ExpandArrayPlaceholder
            if ($ExpandArrayPlaceholder -and $resolvedItem -is [System.Collections.IEnumerable] -and -not ($resolvedItem -is [string])) {
                foreach ($resolvedChild in @($resolvedItem)) {
                    $items.Add($resolvedChild)
                }
            }
            else {
                $items.Add($resolvedItem)
            }
        }
        return @($items)
    }

    return $Node
}

function Resolve-McpConfig([System.Collections.IDictionary]$Config, [System.Collections.IDictionary]$Values) {
    $resolved = [ordered]@{}
    foreach ($key in $Config.Keys) {
        $resolved[$key] = Resolve-McpNode $Config[$key] $Values -ExpandArrayPlaceholder:($key -eq 'args')
    }
    return $resolved
}

function Test-McpProviderReady([System.Collections.IDictionary]$ProviderEntry, [System.Collections.IDictionary]$Values) {
    if (-not $ProviderEntry.Contains('requiredEnv')) { return $true }

    foreach ($name in @($ProviderEntry['requiredEnv'])) {
        if ([string]::IsNullOrWhiteSpace((Get-ConfiguredValue $Values ([string]$name)))) {
            return $false
        }
    }

    return $true
}

function ConvertTo-TomlString([string]$Value) {
    $escaped = $Value.Replace('\', '\\').Replace('"', '\"')
    return '"' + $escaped + '"'
}

function ConvertTo-TomlArray([object[]]$Values) {
    return '[' + (($Values | ForEach-Object { ConvertTo-TomlString ([string]$_) }) -join ', ') + ']'
}

function ConvertTo-CodexMcpSection([string]$ServerName, [System.Collections.IDictionary]$Config) {
    $lines = @("[mcp_servers.$ServerName]")

    if ($Config.Contains('url')) {
        $lines += 'url = ' + (ConvertTo-TomlString ([string]$Config['url']))
    }
    else {
        $lines += 'command = ' + (ConvertTo-TomlString ([string]$Config['command']))
        if ($Config.Contains('args') -and @($Config['args']).Count -gt 0) {
            $lines += 'args = ' + (ConvertTo-TomlArray @($Config['args']))
        }
    }

    if ($Config.Contains('env') -and $Config['env'] -is [System.Collections.IDictionary] -and $Config['env'].Count -gt 0) {
        $lines += ''
        $lines += "[mcp_servers.$ServerName.env]"
        foreach ($envKey in ($Config['env'].Keys | Sort-Object)) {
            $lines += $envKey + ' = ' + (ConvertTo-TomlString ([string]$Config['env'][$envKey]))
        }
    }

    return ($lines -join "`r`n")
}

function Get-DefaultPrimaryRuntime([string[]]$SelectedRuntimes) {
    foreach ($preferred in @('copilot', 'gemini', 'codex', 'claude')) {
        if ($SelectedRuntimes -contains $preferred) {
            return $preferred
        }
    }

    return $null
}

function ConvertFrom-MultiSelectAnswer([string]$Answer, [int]$MaxIndex) {
    $selected = [System.Collections.Generic.List[int]]::new()
    if ([string]::IsNullOrWhiteSpace($Answer)) {
        return @()
    }

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

function Read-RuntimeSelection([object[]]$Catalog, [string[]]$DefaultSelection, [string]$PromptReason) {
    Write-Host '  [PROMPT] Select the AI runtimes where GAL should install machine-level integration.'
    if (-not [string]::IsNullOrWhiteSpace($PromptReason)) {
        Write-Host ('  [INFO] {0}' -f $PromptReason)
    }
    for ($index = 0; $index -lt $Catalog.Count; $index++) {
        $runtime = $Catalog[$index]
        Write-Host ('    {0}. {1} - {2}' -f ($index + 1), $runtime.Label, $runtime.Description)
    }

    $defaultNumbers = @()
    for ($index = 0; $index -lt $Catalog.Count; $index++) {
        if ($DefaultSelection -contains $Catalog[$index].Key) {
            $defaultNumbers += ($index + 1)
        }
    }

    while ($true) {
        $answer = Read-Host ('  [PROMPT] Enter selection numbers (for example: 1,3) [default: {0}]' -f ($defaultNumbers -join ','))
        if ([string]::IsNullOrWhiteSpace($answer)) {
            return @($DefaultSelection)
        }

        try {
            $selectedIndices = ConvertFrom-MultiSelectAnswer -Answer $answer -MaxIndex $Catalog.Count
            if ($selectedIndices.Count -eq 0) {
                Write-Host '  [WARN] Select at least one runtime.' -ForegroundColor Yellow
                continue
            }

            return @($selectedIndices | ForEach-Object { $Catalog[$_ - 1].Key })
        }
        catch {
            Write-Host ('  [WARN] {0}' -f $_.Exception.Message) -ForegroundColor Yellow
        }
    }
}

function Read-PrimaryRuntime([object[]]$Catalog, [string[]]$SelectedRuntimes, [string]$DefaultPrimaryRuntime) {
    $choices = @($Catalog | Where-Object { $SelectedRuntimes -contains $_.Key })
    Write-Host '  [PROMPT] Choose the primary runtime GAL should treat as your default entry point.'
    Write-Host '  [INFO] The repo remains the single source of truth for agents, skills, and commands; this choice affects defaults and summaries only.'
    for ($index = 0; $index -lt $choices.Count; $index++) {
        $runtime = $choices[$index]
        $marker = if ($runtime.Key -eq $DefaultPrimaryRuntime) { ' (default)' } else { '' }
        Write-Host ('    {0}. {1}{2}' -f ($index + 1), $runtime.Label, $marker)
    }

    $defaultIndex = @($choices.Key).IndexOf($DefaultPrimaryRuntime) + 1
    if ($defaultIndex -le 0) { $defaultIndex = 1 }

    while ($true) {
        $answer = Read-Host ('  [PROMPT] Enter one number [default: {0}]' -f $defaultIndex)
        if ([string]::IsNullOrWhiteSpace($answer)) {
            return $choices[$defaultIndex - 1].Key
        }

        $selectedIndex = 0
        if (-not [int]::TryParse($answer.Trim(), [ref]$selectedIndex)) {
            Write-Host '  [WARN] Enter a valid number.' -ForegroundColor Yellow
            continue
        }
        if ($selectedIndex -lt 1 -or $selectedIndex -gt $choices.Count) {
            Write-Host '  [WARN] Selection is out of range.' -ForegroundColor Yellow
            continue
        }

        return $choices[$selectedIndex - 1].Key
    }
}

function Get-DetectedRuntimeSelection {
    $detected = [System.Collections.Generic.List[string]]::new()

    $copilotInstalled = (Test-GalRepoLink $galRootCopilot) -or
        (Get-ChildItem $agentsTarget -Filter '*.agent.md' -File -ErrorAction SilentlyContinue | Where-Object { Test-GalRepoLink $_.FullName } | Select-Object -First 1) -or
        (Get-ChildItem $skillsTarget -Directory -ErrorAction SilentlyContinue | Where-Object { Test-GalRepoLink $_.FullName } | Select-Object -First 1)
    if ($copilotInstalled) { $detected.Add('copilot') }

    $geminiInstalled = (Test-GalRepoLink $galRootGemini) -or
        (Test-GalManagedFile $geminiContextFile) -or
        (Get-ChildItem $geminiCommandsTarget -Filter '*.toml' -File -ErrorAction SilentlyContinue | Where-Object { Test-GalManagedFile $_.FullName } | Select-Object -First 1)
    if ($geminiInstalled) { $detected.Add('gemini') }

    $codexInstalled = (Get-ChildItem $codexSkillsTarget -Directory -ErrorAction SilentlyContinue | Where-Object { Test-GalRepoLink $_.FullName } | Select-Object -First 1)
    if ($codexInstalled) { $detected.Add('codex') }

    $claudeInstalled =
        (Get-ChildItem $claudeSkillsTarget -Directory -ErrorAction SilentlyContinue | Where-Object { Test-GalRepoLink $_.FullName } | Select-Object -First 1) -or
        (Get-ChildItem $claudeCommandsTarget -Filter '*.md' -File -ErrorAction SilentlyContinue | Where-Object { Test-GalManagedFile $_.FullName } | Select-Object -First 1)
    if ($claudeInstalled) { $detected.Add('claude') }

    return @($detected)
}

function Get-InstallSelectionState {
    $state = if (Test-Path $installStateFile) { Read-JsonOrderedMap $installStateFile } else { $null }
    if ($state -is [System.Collections.IDictionary] -and -not $Reconfigure) {
        $selected = @($state['selectedRuntimes']) | Where-Object { $_ -in $runtimeCatalog.Key }
        $primary = [string]$state['primaryRuntime']
        if ($selected.Count -gt 0 -and $selected -contains $primary) {
            Write-Host ''
            Write-Host '=== GAL runtime selection ==='
            Write-Host ('  [OK] Using saved install state from {0}' -f $installStateFile)
            Write-Host ('  [OK] Selected runtimes: {0}' -f ($selected -join ', '))
            Write-Host ('  [OK] Primary runtime: {0}' -f $primary)
            return [pscustomobject]@{
                SelectedRuntimes = $selected
                PrimaryRuntime = $primary
                Mode = 'saved-state'
            }
        }
    }

    $detected = Get-DetectedRuntimeSelection
    $defaultSelection = if ($detected.Count -gt 0) { $detected } else { @($runtimeCatalog.Key) }
    $promptReason = if ($detected.Count -gt 0) {
        'Detected a legacy GAL machine install without install-state. Confirm or change the runtime set before continuing.'
    }
    else {
        'First-time GAL install detected. Choose where GAL should install runtime integrations.'
    }

    Write-Host ''
    Write-Host '=== GAL runtime selection ==='
    $selectedRuntimes = Read-RuntimeSelection -Catalog $runtimeCatalog -DefaultSelection $defaultSelection -PromptReason $promptReason
    $defaultPrimaryRuntime = if ($state -is [System.Collections.IDictionary] -and ($selectedRuntimes -contains [string]$state['primaryRuntime'])) {
        [string]$state['primaryRuntime']
    }
    else {
        Get-DefaultPrimaryRuntime -SelectedRuntimes $selectedRuntimes
    }
    $primaryRuntime = Read-PrimaryRuntime -Catalog $runtimeCatalog -SelectedRuntimes $selectedRuntimes -DefaultPrimaryRuntime $defaultPrimaryRuntime

    if (-not $DryRun) {
        Write-JsonOrderedMap $installStateFile ([ordered]@{
            schemaVersion = 1
            selectedRuntimes = @($selectedRuntimes)
            primaryRuntime = $primaryRuntime
            installedAt = if ($state -is [System.Collections.IDictionary] -and $state.Contains('installedAt')) { $state['installedAt'] } else { (Get-Date -Format 'o') }
            lastConfiguredAt = (Get-Date -Format 'o')
        })
        Write-Host ('  [OK] Saved install state: {0}' -f $installStateFile)
    }
    else {
        Write-Host ('  [DRY RUN] Would save install state: {0}' -f $installStateFile)
    }

    return [pscustomobject]@{
        SelectedRuntimes = @($selectedRuntimes)
        PrimaryRuntime = $primaryRuntime
        Mode = if ($detected.Count -gt 0) { 'legacy-migration' } else { 'fresh-install' }
    }
}

if (-not $Uninstall) {
    Ensure-Ripgrep
}

$selectedRuntimes = @()
$primaryRuntime = $null
if (-not $Uninstall) {
    $installSelection = Get-InstallSelectionState
    $selectedRuntimes = @($installSelection.SelectedRuntimes)
    $primaryRuntime = $installSelection.PrimaryRuntime
}

$installCopilot = $selectedRuntimes -contains 'copilot'
$installGemini = $selectedRuntimes -contains 'gemini'
$installCodex = $selectedRuntimes -contains 'codex'
$installClaude = $selectedRuntimes -contains 'claude'
$installSharedSkills = $installGemini -or $installCodex
$needsBakedCommandSkills = $installCopilot -or $installGemini -or $installCodex -or $installClaude

# --- Ensure target directories ---

if (-not $Uninstall) {
    $dirsToEnsure = [System.Collections.Generic.List[string]]::new()
    foreach ($dir in @($galStateRoot)) {
        if (-not [string]::IsNullOrWhiteSpace($dir)) {
            $dirsToEnsure.Add($dir)
        }
    }
    if ($installCopilot) {
        foreach ($dir in @($copilotRoot, $agentsTarget, $skillsTarget)) {
            $dirsToEnsure.Add($dir)
        }
    }
    if ($installSharedSkills) {
        foreach ($dir in @($sharedAgentsRoot, $sharedSkillsTarget)) {
            $dirsToEnsure.Add($dir)
        }
    }
    if ($installGemini) {
        foreach ($dir in @($geminiRoot, $geminiSkillsTarget, $geminiCommandsTarget)) {
            $dirsToEnsure.Add($dir)
        }
    }
    if ($installCodex) {
        foreach ($dir in @($codexRoot, $codexSkillsTarget)) {
            $dirsToEnsure.Add($dir)
        }
    }
    if ($installClaude) {
        foreach ($dir in @($claudeRoot, $claudeSkillsTarget, $claudeCommandsTarget)) {
            $dirsToEnsure.Add($dir)
        }
    }

    foreach ($dir in ($dirsToEnsure | Select-Object -Unique)) {
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

    if ($Uninstall -or -not $installCopilot) {
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

    if ($Uninstall -or -not $installCopilot) {
        Remove-SafeLink $linkPath
    }
    else {
        if (New-SafeSymlink $linkPath $d.FullName "Directory") { $skillOk++ } else { $skillFail++ }
    }
}

# --- Migration: remove .gemini/skills GAL symlinks (Gemini now uses custom commands + shared .agents skills) ---

Write-Host ""
Write-Host "=== Migration: .gemini/skills cleanup ==="

$geminiMigrateOk = 0
$allGalSkillNames = @($skillDirs | ForEach-Object { $_.Name }) + $activeCommandSkillNames + @('gal.bak')
foreach ($name in $allGalSkillNames) {
    $linkPath = Join-Path $geminiSkillsTarget $name
    if (-not (Test-Path $linkPath)) { continue }
    $item = Get-Item $linkPath -Force -ErrorAction SilentlyContinue
    if ($null -eq $item) { continue }
    $isReparsePoint = ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0
    $isGalLink = $isReparsePoint -and (($item.Target -join ';') -like "*$repoRoot*")
    $isGalBak  = ($name -eq 'gal.bak')
    if ($isGalLink -or $isGalBak) {
        if ($DryRun) {
            Write-Host "  [DRY RUN] Would remove: $linkPath"
        } else {
            Remove-Item $linkPath -Recurse -Force
            Write-Host "  [REMOVED] $linkPath"
            $geminiMigrateOk++
        }
    }
}
if (-not $DryRun -and $geminiMigrateOk -eq 0) {
    Write-Host "  [OK] Nothing to migrate in .gemini/skills"
}

# --- Shared reusable skill symlinks (.agents/skills for Gemini + Codex) ---

Write-Host ""
Write-Host "=== Shared Skills - Gemini + Codex ($($skillDirs.Count) reusable directories via .agents) ==="

$sharedSkillOk = 0
$sharedSkillFail = 0

foreach ($d in $skillDirs) {
    $linkPath = Join-Path $sharedSkillsTarget $d.Name

    if ($Uninstall -or -not $installSharedSkills) {
        Remove-SafeLink $linkPath
    }
    else {
        if (New-SafeSymlink $linkPath $d.FullName "Directory") { $sharedSkillOk++ } else { $sharedSkillFail++ }
    }
}

# --- Claude reusable skill symlinks (.claude/skills) ---

Write-Host ""
Write-Host "=== Claude Skills ($($skillDirs.Count) reusable directories) ==="

$claudeSkillOk = 0
$claudeSkillFail = 0

foreach ($d in $skillDirs) {
    $linkPath = Join-Path $claudeSkillsTarget $d.Name

    if ($Uninstall -or -not $installClaude) {
        Remove-SafeLink $linkPath
    }
    else {
        if (New-SafeSymlink $linkPath $d.FullName "Directory") { $claudeSkillOk++ } else { $claudeSkillFail++ }
    }
}

# --- Gemini gal-context.md ---

Write-Host ""
Write-Host "=== Gemini gal-context.md ==="

if ($Uninstall -or -not $installGemini) {
    if (Test-Path $geminiContextFile) {
        if ($DryRun) {
            Write-Host "  [DRY RUN] Would remove: $geminiContextFile"
        }
        else {
            Remove-Item $geminiContextFile -Force
            Write-Host "  [REMOVED] $geminiContextFile"
        }
    }
}
else {
    $skillImports = $skillDirs | Sort-Object Name | ForEach-Object {
        $skillMd = Join-Path $sharedSkillsTarget $_.Name "SKILL.md"
        "@$skillMd"
    }
    $contextContent = ($skillImports -join "`n") + "`n"

    if ($DryRun) {
        Write-Host "  [DRY RUN] Would write: $geminiContextFile ($($skillDirs.Count) skill imports)"
    }
    else {
        [System.IO.File]::WriteAllText($geminiContextFile, $contextContent, $utf8NoBom)
        Write-Host "  [OK] $geminiContextFile ($($skillDirs.Count) skill imports)"
    }
}

# --- Gemini settings.json: context.fileName bridge ---

Write-Host ""
Write-Host "=== Gemini settings.json bridge ==="

if ($Uninstall) {
    Write-Host "  [SKIP] settings.json not modified during uninstall (user-owned file)"
} elseif (-not $installGemini) {
    Write-Host "  [SKIP] Gemini runtime not selected; settings.json bridge not updated"
} elseif ($DryRun) {
    Write-Host "  [DRY RUN] Would merge AGENTS.md into context.fileName in: $geminiSettingsFile"
} else {
    if (Test-Path $geminiSettingsFile) {
        $rawJson = Get-Content $geminiSettingsFile -Raw -Encoding UTF8
        try   { $settings = $rawJson | ConvertFrom-Json }
        catch { Write-Host "  [WARN] Could not parse $geminiSettingsFile as JSON — skipping bridge" -ForegroundColor Yellow; $settings = $null }
    } else {
        $settings = [PSCustomObject]@{}
    }

    if ($null -ne $settings) {
        if (-not (Get-Member -InputObject $settings -Name 'context' -MemberType NoteProperty)) {
            Add-Member -InputObject $settings -MemberType NoteProperty -Name 'context' -Value ([PSCustomObject]@{})
        }
        if (-not (Get-Member -InputObject $settings.context -Name 'fileName' -MemberType NoteProperty)) {
            Add-Member -InputObject $settings.context -MemberType NoteProperty -Name 'fileName' -Value @('AGENTS.md', 'GEMINI.md')
        } else {
            $current = @($settings.context.fileName)
            foreach ($required in @('AGENTS.md', 'GEMINI.md')) {
                if ($current -notcontains $required) { $current += $required }
            }
            $settings.context.fileName = $current
        }
        [System.IO.File]::WriteAllText($geminiSettingsFile, ($settings | ConvertTo-Json -Depth 10), $utf8NoBom)
        Write-Host "  [OK] $geminiSettingsFile (context.fileName includes AGENTS.md and GEMINI.md)"
    }
}

# --- VS Code settings.json: disable duplicate .agents skill discovery ---

Write-Host ""
Write-Host "=== VS Code settings bridge ==="

if ($Uninstall) {
    Write-Host "  [SKIP] VS Code settings.json not modified during uninstall (user-owned file)"
}
elseif (-not $installCopilot) {
    Write-Host "  [SKIP] Copilot runtime not selected; VS Code settings bridge not updated"
}
elseif ($DryRun) {
    Write-Host "  [DRY RUN] Would set chat.agentSkillsLocations['~/.agents/skills']=false in: $vscodeSettingsFile"
}
else {
    if (Test-Path $vscodeSettingsFile) {
        $rawJson = Get-Content $vscodeSettingsFile -Raw -Encoding UTF8
        try   { $settings = $rawJson | ConvertFrom-Json }
        catch { Write-Host "  [WARN] Could not parse $vscodeSettingsFile as JSON — add chat.agentSkillsLocations manually" -ForegroundColor Yellow; $settings = $null }
    } else {
        $settingsDir = Split-Path $vscodeSettingsFile -Parent
        if (-not (Test-Path $settingsDir)) {
            New-Item -ItemType Directory -Path $settingsDir -Force | Out-Null
        }
        $settings = [PSCustomObject]@{}
    }

    if ($null -ne $settings) {
        if (-not (Get-Member -InputObject $settings -Name 'chat.agentSkillsLocations' -MemberType NoteProperty)) {
            Add-Member -InputObject $settings -MemberType NoteProperty -Name 'chat.agentSkillsLocations' -Value ([PSCustomObject]@{})
        }

        $skillLocations = $settings.'chat.agentSkillsLocations'
        if ($skillLocations -is [System.Collections.IDictionary]) {
            $skillLocations['~/.agents/skills'] = $false
        }
        elseif ($skillLocations -is [PSCustomObject]) {
            if (Get-Member -InputObject $skillLocations -Name '~/.agents/skills' -MemberType NoteProperty) {
                $skillLocations.'~/.agents/skills' = $false
            }
            else {
                Add-Member -InputObject $skillLocations -MemberType NoteProperty -Name '~/.agents/skills' -Value $false
            }
        }
        else {
            Write-Host "  [WARN] chat.agentSkillsLocations is not an object in $vscodeSettingsFile — skipping" -ForegroundColor Yellow
            $skillLocations = $null
        }

        if ($null -ne $skillLocations) {
            [System.IO.File]::WriteAllText($vscodeSettingsFile, ($settings | ConvertTo-Json -Depth 10), $utf8NoBom)
            Write-Host "  [OK] $vscodeSettingsFile (chat.agentSkillsLocations disables ~/.agents/skills for VS Code)"
        }
    }
}

# --- MCP config bridge ---

Write-Host ""
Write-Host "=== MCP config bridge ==="

if ($Uninstall) {
    Write-Host "  [SKIP] MCP config files not modified during uninstall (user-owned files)"
}
elseif ($DryRun) {
    if ($installCopilot) {
        Write-Host "  [DRY RUN] Would merge MCP servers into: $vscodeMcpFile"
    }
    if ($installGemini) {
        Write-Host "  [DRY RUN] Would merge MCP servers into: $geminiSettingsFile"
    }
    if ($installCodex) {
        Write-Host "  [DRY RUN] Would merge MCP servers into: $codexConfigFile"
    }
    if ($installClaude) {
        Write-Host "  [SKIP] Claude Code MCP merge remains deferred in this installer"
    }
}
else {
    if (-not (Test-Path $mcpManifestLocalFile)) {
        Write-JsonOrderedMap $mcpManifestLocalFile ([ordered]@{ servers = [ordered]@{} })
        Write-Host "  [OK] Created local MCP override file: $mcpManifestLocalFile"
    }

    $manifest = Read-JsonOrderedMap $mcpManifestExampleFile
    if ($null -ne $manifest -and (Test-Path $mcpManifestLocalFile)) {
        $localManifest = Read-JsonOrderedMap $mcpManifestLocalFile
        if ($null -ne $localManifest) {
            $manifest = Merge-OrderedMap $manifest $localManifest
        }
        else {
            $manifest = $null
        }
    }

    if ($null -ne $manifest -and $manifest.Contains('servers') -and $manifest['servers'] -is [System.Collections.IDictionary]) {
        $mcpVariables = Get-McpVariableMap (Read-KeyValueEnvFile (Join-Path $repoRoot 'config.local.env'))

        $vscodeMcp = Read-JsonOrderedMap $vscodeMcpFile
        if ($installCopilot -and $null -ne $vscodeMcp) {
            if (-not $vscodeMcp.Contains('servers') -or $vscodeMcp['servers'] -isnot [System.Collections.IDictionary]) {
                $vscodeMcp['servers'] = [ordered]@{}
            }

            $vscodeChanged = $false
            foreach ($serverName in $manifest['servers'].Keys) {
                $server = $manifest['servers'][$serverName]
                if (-not ($server.Contains('providers') -and $server['providers'].Contains('vscode'))) { continue }

                $provider = $server['providers']['vscode']
                if ($provider['enabled'] -ne $true) { continue }
                if (-not (Test-McpProviderReady $provider $mcpVariables)) {
                    Write-Host "  [WARN] Skipping VS Code MCP server '$serverName' because required env is missing" -ForegroundColor Yellow
                    continue
                }

                $providerKey = if ($provider.Contains('key')) { [string]$provider['key'] } else { [string]$serverName }
                if ($vscodeMcp['servers'].Contains($providerKey)) { continue }

                $vscodeMcp['servers'][$providerKey] = Resolve-McpConfig $provider['config'] $mcpVariables
                $vscodeChanged = $true
                Write-Host "  [ADD] VS Code MCP server: $providerKey"
            }

            if ($vscodeChanged) {
                Write-JsonOrderedMap $vscodeMcpFile $vscodeMcp
                Write-Host "  [OK] $vscodeMcpFile"
            }
        }

        $geminiSettings = Read-JsonOrderedMap $geminiSettingsFile
        if ($installGemini -and $null -ne $geminiSettings) {
            if (-not $geminiSettings.Contains('mcpServers') -or $geminiSettings['mcpServers'] -isnot [System.Collections.IDictionary]) {
                $geminiSettings['mcpServers'] = [ordered]@{}
            }

            $geminiChanged = $false
            foreach ($serverName in $manifest['servers'].Keys) {
                $server = $manifest['servers'][$serverName]
                if (-not ($server.Contains('providers') -and $server['providers'].Contains('gemini'))) { continue }

                $provider = $server['providers']['gemini']
                if ($provider['enabled'] -ne $true) { continue }
                if (-not (Test-McpProviderReady $provider $mcpVariables)) {
                    Write-Host "  [WARN] Skipping Gemini MCP server '$serverName' because required env is missing" -ForegroundColor Yellow
                    continue
                }

                $providerKey = if ($provider.Contains('key')) { [string]$provider['key'] } else { [string]$serverName }
                if ($geminiSettings['mcpServers'].Contains($providerKey)) { continue }

                $geminiSettings['mcpServers'][$providerKey] = Resolve-McpConfig $provider['config'] $mcpVariables
                $geminiChanged = $true
                Write-Host "  [ADD] Gemini MCP server: $providerKey"
            }

            if ($geminiChanged) {
                Write-JsonOrderedMap $geminiSettingsFile $geminiSettings
                Write-Host "  [OK] $geminiSettingsFile"
            }
        }

        if ($installCodex) {
            $codexConfigDir = Split-Path $codexConfigFile -Parent
            if (-not (Test-Path $codexConfigDir)) {
                New-Item -ItemType Directory -Path $codexConfigDir -Force | Out-Null
            }

            $codexRaw = if (Test-Path $codexConfigFile) { Get-Content $codexConfigFile -Raw -Encoding UTF8 } else { '' }
            $codexSections = @()

            foreach ($serverName in $manifest['servers'].Keys) {
                $server = $manifest['servers'][$serverName]
                if (-not ($server.Contains('providers') -and $server['providers'].Contains('codex'))) { continue }

                $provider = $server['providers']['codex']
                if ($provider['enabled'] -ne $true) { continue }
                if (-not (Test-McpProviderReady $provider $mcpVariables)) {
                    Write-Host "  [WARN] Skipping Codex MCP server '$serverName' because required env is missing" -ForegroundColor Yellow
                    continue
                }

                $providerKey = if ($provider.Contains('key')) { [string]$provider['key'] } else { [string]$serverName }
                $pattern = '(?m)^\[mcp_servers\.' + [regex]::Escape($providerKey) + '\]\s*$'
                if ($codexRaw -match $pattern) { continue }

                $resolvedConfig = Resolve-McpConfig $provider['config'] $mcpVariables
                $codexSections += ConvertTo-CodexMcpSection $providerKey $resolvedConfig
                Write-Host "  [ADD] Codex MCP server: $providerKey"
            }

            if ($codexSections.Count -gt 0) {
                $newCodexRaw = $codexRaw.TrimEnd("`r", "`n")
                if (-not [string]::IsNullOrWhiteSpace($newCodexRaw)) {
                    $newCodexRaw += "`r`n`r`n"
                }
                $newCodexRaw += ($codexSections -join "`r`n`r`n") + "`r`n"
                [System.IO.File]::WriteAllText($codexConfigFile, $newCodexRaw, $utf8NoBom)
                Write-Host "  [OK] $codexConfigFile"
            }
        }

        if ($installClaude) {
            Write-Host "  [SKIP] Claude Code MCP merge remains deferred in this installer"
        }
    }
}

$galRootOk = 0
Write-Host ""
Write-Host "=== GAL_ROOT symlinks ==="

if ($Uninstall -or -not $installCopilot) {
    Remove-SafeLink $galRootCopilot
}
else {
    if (New-SafeSymlink $galRootCopilot $repoRoot "Directory") { $galRootOk++ }
}

if ($Uninstall -or -not $installGemini) {
    Remove-SafeLink $galRootGemini
}
else {
    if (New-SafeSymlink $galRootGemini  $repoRoot "Directory") { $galRootOk++ }
}

# --- Generated GAL command skills (bake templates → commands/gal*/SKILL.md) ---

Write-Host ""
Write-Host "=== Generated GAL command skills ==="

if ($Uninstall -or -not $needsBakedCommandSkills) {
    foreach ($commandSkill in $commandSkillDirs) {
        $bakedSkill = Join-Path $commandSkill.Source "SKILL.md"
        if (Test-Path $bakedSkill) {
            if ($DryRun) { Write-Host "  [DRY RUN] Would remove baked: $bakedSkill" }
            else { Remove-Item $bakedSkill -Force; Write-Host "  [REMOVED] $bakedSkill" }
        }
    }
} else {
    foreach ($commandSkill in $commandSkillDirs) {
        if (-not (Test-Path $commandSkill.Template)) {
            Write-Host "  [WARN] Template not found: $($commandSkill.Template)"
        } else {
            $localOverridePath = Join-Path $commandSkill.Source 'SKILL.local.md'
            $baked = Get-BakedCommandSkillContent -TemplatePath $commandSkill.Template -LocalOverridePath $localOverridePath
            $bakedSkill = Join-Path $commandSkill.Source "SKILL.md"
            if ($DryRun) {
                if (Test-Path $localOverridePath) {
                    Write-Host "  [DRY RUN] Would write baked with local overlay: $bakedSkill"
                }
                else {
                    Write-Host "  [DRY RUN] Would write baked: $bakedSkill"
                }
            } else {
                [System.IO.File]::WriteAllText($bakedSkill, $baked, $utf8NoBom)
                if (Test-Path $localOverridePath) {
                    Write-Host "  [OK] $bakedSkill (with local overlay)"
                }
                else {
                    Write-Host "  [OK] $bakedSkill"
                }
            }
        }
    }
}

# --- GAL command skill symlinks (commands/gal*/ → ~/.copilot/skills/gal*/ + ~/.codex/skills/gal*/) ---

Write-Host ""
Write-Host "=== GAL command skill symlinks ==="

if ($Uninstall) {
    foreach ($commandSkill in $commandSkillDirs) {
        Remove-SafeLink $commandSkill.CopilotTarget
        Remove-SafeLink $commandSkill.CodexTarget
    }
} else {
    foreach ($commandSkill in $commandSkillDirs) {
        if ($installCopilot) {
            New-SafeSymlink $commandSkill.CopilotTarget $commandSkill.Source "Directory" | Out-Null
        }
        else {
            Remove-SafeLink $commandSkill.CopilotTarget
        }

        if ($installCodex) {
            New-SafeSymlink $commandSkill.CodexTarget $commandSkill.Source "Directory" | Out-Null
        }
        else {
            Remove-SafeLink $commandSkill.CodexTarget
        }
    }
}

# --- Migration: remove GAL command skill symlinks from shared .agents/skills ---

Write-Host ""
Write-Host "=== Migration: .agents command cleanup ==="

foreach ($commandSkill in $commandSkillDirs) {
    $sharedCommandPath = Join-Path $sharedSkillsTarget $commandSkill.Name
    if (-not (Test-Path $sharedCommandPath)) { continue }

    $item = Get-Item $sharedCommandPath -Force -ErrorAction SilentlyContinue
    if ($null -eq $item) { continue }

    if (-not (Test-GalRepoLink $sharedCommandPath)) {
        Write-Host "  [SKIP] User-owned shared skill preserved: $sharedCommandPath"
        continue
    }

    if ($DryRun) {
        Write-Host "  [DRY RUN] Would remove: $sharedCommandPath"
    }
    else {
        Remove-Item $sharedCommandPath -Recurse -Force
        Write-Host "  [REMOVED] $sharedCommandPath"
    }
}

# --- Gemini custom commands ---

Write-Host ""
Write-Host "=== Gemini custom commands ==="

if ($Uninstall -or -not $installGemini) {
    foreach ($commandSkill in $commandSkillDirs) {
        $commandFile = Join-Path $geminiCommandsTarget ("{0}.toml" -f $commandSkill.Name)
        if (-not (Test-Path $commandFile)) { continue }
        if (-not (Test-GalManagedFile $commandFile)) {
            Write-Host "  [SKIP] User-owned Gemini command preserved: $commandFile"
            continue
        }
        if ($DryRun) {
            Write-Host "  [DRY RUN] Would remove: $commandFile"
        }
        else {
            Remove-Item $commandFile -Force
            Write-Host "  [REMOVED] $commandFile"
        }
    }
}
else {
    foreach ($commandSkill in $commandSkillDirs) {
        $commandFile = Join-Path $geminiCommandsTarget ("{0}.toml" -f $commandSkill.Name)
        $commandContent = New-GeminiCommandFileContent (Join-Path $commandSkill.Source 'SKILL.md')
        if ($DryRun) {
            Write-Host "  [DRY RUN] Would write: $commandFile"
        }
        else {
            [System.IO.File]::WriteAllText($commandFile, $commandContent, $utf8NoBom)
            Write-Host "  [OK] $commandFile"
        }
    }

    Write-Host "  [NOTE] Reload active Gemini sessions with /commands reload or restart Gemini CLI to pick up updated GAL commands."
}

# --- Claude custom commands ---

Write-Host ""
Write-Host "=== Claude custom commands ==="

if ($Uninstall -or -not $installClaude) {
    foreach ($commandSkill in $commandSkillDirs) {
        $commandFile = Join-Path $claudeCommandsTarget ("{0}.md" -f $commandSkill.Name)
        if (-not (Test-Path $commandFile)) { continue }
        if (-not (Test-GalManagedFile $commandFile)) {
            Write-Host "  [SKIP] User-owned Claude command preserved: $commandFile"
            continue
        }
        if ($DryRun) {
            Write-Host "  [DRY RUN] Would remove: $commandFile"
        }
        else {
            Remove-Item $commandFile -Force
            Write-Host "  [REMOVED] $commandFile"
        }
    }
}
else {
    foreach ($commandSkill in $commandSkillDirs) {
        $commandFile = Join-Path $claudeCommandsTarget ("{0}.md" -f $commandSkill.Name)
        $commandContent = New-ClaudeCommandFileContent -SkillPath (Join-Path $commandSkill.Source 'SKILL.md') -CommandName $commandSkill.Name
        if ($DryRun) {
            Write-Host "  [DRY RUN] Would write: $commandFile"
        }
        else {
            [System.IO.File]::WriteAllText($commandFile, $commandContent, $utf8NoBom)
            Write-Host "  [OK] $commandFile"
        }
    }

    Write-Host "  [NOTE] Restart Claude Code or reload its command surface to pick up updated GAL commands."
}

# --- Migration: remove obsolete command files from installed locations ---

Write-Host ""
Write-Host "=== Migration: obsolete command cleanup ==="

foreach ($skillsDir in @($skillsTarget, $geminiSkillsTarget, $sharedSkillsTarget, $codexSkillsTarget)) {
    $obsoleteCommandLinks = Get-ChildItem $skillsDir -Directory -ErrorAction SilentlyContinue | Where-Object {
        $_.Name -notin $activeCommandSkillNames -and (Test-GalCommandLink $_.FullName)
    }

    foreach ($d in $obsoleteCommandLinks) {
        if ($DryRun) {
            Write-Host "  [DRY RUN] Would remove obsolete command link: $($d.FullName)"
        }
        else {
            Remove-Item $d.FullName -Recurse -Force
            Write-Host "  [REMOVED] Obsolete command link: $($d.FullName)"
        }
    }
}

$obsoleteGeminiCommands = Get-ChildItem $geminiCommandsTarget -Filter '*.toml' -File -ErrorAction SilentlyContinue | Where-Object {
    $_.BaseName -notin $activeCommandSkillNames -and (Test-GalManagedFile $_.FullName)
}

foreach ($commandFile in $obsoleteGeminiCommands) {
    if ($DryRun) {
        Write-Host "  [DRY RUN] Would remove obsolete Gemini command: $($commandFile.FullName)"
    }
    else {
        Remove-Item $commandFile.FullName -Force
        Write-Host "  [REMOVED] Obsolete Gemini command: $($commandFile.FullName)"
    }
}

$obsoleteClaudeCommands = Get-ChildItem $claudeCommandsTarget -Filter '*.md' -File -ErrorAction SilentlyContinue | Where-Object {
    $_.BaseName -notin $activeCommandSkillNames -and (Test-GalManagedFile $_.FullName)
}

foreach ($commandFile in $obsoleteClaudeCommands) {
    if ($DryRun) {
        Write-Host "  [DRY RUN] Would remove obsolete Claude command: $($commandFile.FullName)"
    }
    else {
        Remove-Item $commandFile.FullName -Force
        Write-Host "  [REMOVED] Obsolete Claude command: $($commandFile.FullName)"
    }
}

# --- Migration: remove legacy gal-* dirs from installed locations ---

Write-Host ""
Write-Host "=== Migration: gal-* cleanup ==="

foreach ($skillsDir in @($skillsTarget, $geminiSkillsTarget, $sharedSkillsTarget, $codexSkillsTarget)) {
    $legacyDirs = Get-ChildItem $skillsDir -Directory -ErrorAction SilentlyContinue | Where-Object {
        $_.Name -like 'gal-*' -and $_.Name -notin $activeCommandSkillNames
    }
    foreach ($d in $legacyDirs) {
        if ($DryRun) {
            Write-Host "  [DRY RUN] Would remove: $($d.FullName)"
        } else {
            Remove-Item $d.FullName -Recurse -Force
            Write-Host "  [REMOVED] $($d.FullName)"
        }
    }
}

# --- Summary ---

Write-Host ""
if ($Uninstall) {
    Write-Host "Uninstall complete."
}
elseif ($DryRun) {
    Write-Host "Dry run complete. No changes made."
    Write-Host ('Selected runtimes: {0}' -f ($selectedRuntimes -join ', '))
    Write-Host ('Primary runtime: {0}' -f $primaryRuntime)
}
else {
    Write-Host ('Setup complete: runtimes={0}; primary={1}; agents={2}/{3}; skills(copilot)={4}/{5}; skills(shared)={6}/{7}; skills(claude)={8}/{9}; gal-root={10}/2' -f ($selectedRuntimes -join ', '), $primaryRuntime, $agentOk, $agentFiles.Count, $skillOk, $skillDirs.Count, $sharedSkillOk, $skillDirs.Count, $claudeSkillOk, $skillDirs.Count, $galRootOk)
    Write-Host "Note: If SKILL.template.md or SKILL.local.md changes, re-run Setup-Machine.ps1 -Replace to regenerate."
    if ($agentFail -gt 0 -or $skillFail -gt 0 -or $sharedSkillFail -gt 0 -or $claudeSkillFail -gt 0) {
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

        # 5. Re-checkout ONLY the smudge-filtered files (not all tracked files)
        if (Test-Path $localEnv) {
            $hasValues = Get-Content $localEnv | Where-Object {
                $_ -notmatch '^\s*#' -and $_ -match '=.+'
            }
            if ($hasValues) {
                $trackedFilterFiles = @(
                    "config.local.env",
                    "model-roles.local.md"
                ) | Where-Object {
                    (git ls-files --error-unmatch $_ 2>$null) -ne $null
                }

                if ($trackedFilterFiles.Count -gt 0) {
                    git checkout -- @trackedFilterFiles
                    Write-Host "  [OK] Re-checked out tracked filtered files (smudge filter applied)"
                }
                else {
                    Write-Host "  [INFO] Filtered files are not tracked yet — skipping git checkout"
                }
            }
            else {
                Write-Host "  [INFO] config.local.env has no values yet — fill it in, then run: git checkout -- config.local.env model-roles.local.md"
            }
        }
    }
    finally {
        Pop-Location
    }
}
