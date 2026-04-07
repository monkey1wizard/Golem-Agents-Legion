<#
.SYNOPSIS
    Creates symlinks from golem-agents-legion repo to Copilot, Gemini, and Codex runtime directories.

.DESCRIPTION
        Links:
            - agent/*.agent.md  → ~/.copilot/agents/*.agent.md
            - skills/*/         → ~/.copilot/skills/*/
            - skills/*/         → ~/.agents/skills/*/ (Gemini CLI + Codex CLI shared)
            - commands/gal/     → ~/.copilot/skills/gal/ + ~/.agents/skills/gal/ (baked dispatcher skill)
            - <repo root>       → ~/.copilot/gal/ + ~/.gemini/gal/ (GAL_ROOT dir symlinks)
      - Generates commands/gal/SKILL.md from SKILL.template.md (baked absolute paths)
      - Generates ~/.gemini/gal-context.md (@file skill imports, paths reference .agents/skills)
      - Merges VS Code user settings so Copilot Chat ignores ~/.agents/skills and does not double-list skills
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

$geminiRoot = Join-Path $env:USERPROFILE ".gemini"
$geminiSkillsTarget = Join-Path $geminiRoot "skills"
$geminiContextFile = Join-Path $geminiRoot "gal-context.md"
$geminiSettingsFile = Join-Path $geminiRoot "settings.json"
$vscodeSettingsFile = Join-Path $env:APPDATA "Code\User\settings.json"
$vscodeMcpFile = Join-Path $env:APPDATA "Code\User\mcp.json"

$codexSkillsRoot = Join-Path $env:USERPROFILE ".agents"
$codexSkillsTarget = Join-Path $codexSkillsRoot "skills"
$codexConfigFile = Join-Path $env:USERPROFILE ".codex\config.toml"

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
    Where-Object { $_.Name -ne 'gal' } |
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
        $items = @()
        foreach ($item in $InputObject) {
            $items += ,(ConvertTo-OrderedMap $item)
        }
        return $items
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

    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
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
        $items = @()
        foreach ($item in $Node) {
            $resolvedItem = Resolve-McpNode $item $Values -ExpandArrayPlaceholder:$ExpandArrayPlaceholder
            if ($ExpandArrayPlaceholder -and $resolvedItem -is [System.Collections.IEnumerable] -and -not ($resolvedItem -is [string])) {
                $items += @($resolvedItem)
            }
            else {
                $items += ,$resolvedItem
            }
        }
        return $items
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

# --- Ensure target directories ---

if (-not $Uninstall) {
    foreach ($dir in @($copilotRoot, $agentsTarget, $skillsTarget, $geminiRoot, $geminiSkillsTarget, $codexSkillsRoot, $codexSkillsTarget)) {
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

# --- Migration: remove .gemini/skills GAL symlinks (Gemini now discovers via .agents/skills) ---

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

# --- Shared Skill symlinks (Gemini + Codex both discover via .agents/skills) ---

Write-Host ""
Write-Host "=== Shared Skills - Gemini + Codex ($($skillDirs.Count) directories via .agents) ==="

$codexSkillOk = 0
$codexSkillFail = 0

foreach ($d in $skillDirs) {
    $linkPath = Join-Path $codexSkillsTarget $d.Name

    if ($Uninstall) {
        Remove-SafeLink $linkPath
    }
    else {
        if (New-SafeSymlink $linkPath $d.FullName "Directory") { $codexSkillOk++ } else { $codexSkillFail++ }
    }
}

# --- Gemini gal-context.md ---

Write-Host ""
Write-Host "=== Gemini gal-context.md ==="

if ($Uninstall) {
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
        $skillMd = Join-Path $codexSkillsTarget $_.Name "SKILL.md"
        "@$skillMd"
    }
    $commandSkillImports = $commandSkillDirs | ForEach-Object {
        "@" + (Join-Path $_.CodexTarget "SKILL.md")
    }
    $allImports = $commandSkillImports + $skillImports
    $contextContent = ($allImports -join "`n") + "`n"

    if ($DryRun) {
        Write-Host "  [DRY RUN] Would write: $geminiContextFile ($($skillDirs.Count + $commandSkillDirs.Count) skill imports)"
    }
    else {
        $utf8NoBom = New-Object System.Text.UTF8Encoding $false
        [System.IO.File]::WriteAllText($geminiContextFile, $contextContent, $utf8NoBom)
        Write-Host "  [OK] $geminiContextFile ($($skillDirs.Count + $commandSkillDirs.Count) skill imports)"
    }
}

# --- Gemini settings.json: context.fileName bridge ---

Write-Host ""
Write-Host "=== Gemini settings.json bridge ==="

if ($Uninstall) {
    Write-Host "  [SKIP] settings.json not modified during uninstall (user-owned file)"
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
        $utf8NoBom = New-Object System.Text.UTF8Encoding $false
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
            $utf8NoBom = New-Object System.Text.UTF8Encoding $false
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
    Write-Host "  [DRY RUN] Would merge MCP servers into: $vscodeMcpFile"
    Write-Host "  [DRY RUN] Would merge MCP servers into: $geminiSettingsFile"
    Write-Host "  [DRY RUN] Would merge MCP servers into: $codexConfigFile"
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
        if ($null -ne $vscodeMcp) {
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
        if ($null -ne $geminiSettings) {
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
            $utf8NoBom = New-Object System.Text.UTF8Encoding $false
            $newCodexRaw = $codexRaw.TrimEnd("`r", "`n")
            if (-not [string]::IsNullOrWhiteSpace($newCodexRaw)) {
                $newCodexRaw += "`r`n`r`n"
            }
            $newCodexRaw += ($codexSections -join "`r`n`r`n") + "`r`n"
            [System.IO.File]::WriteAllText($codexConfigFile, $newCodexRaw, $utf8NoBom)
            Write-Host "  [OK] $codexConfigFile"
        }
    }
}

$galRootOk = 0
Write-Host ""
Write-Host "=== GAL_ROOT symlinks ==="

if ($Uninstall) {
    Remove-SafeLink $galRootCopilot
    Remove-SafeLink $galRootGemini
} else {
    if (New-SafeSymlink $galRootCopilot $repoRoot "Directory") { $galRootOk++ }
    if (New-SafeSymlink $galRootGemini  $repoRoot "Directory") { $galRootOk++ }
}

# --- Generated GAL command skills (bake templates → commands/gal*/SKILL.md) ---

Write-Host ""
Write-Host "=== Generated GAL command skills ==="

if ($Uninstall) {
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
            $baked = (Get-Content $commandSkill.Template -Raw) -replace [regex]::Escape('{{GAL_ROOT}}'), $repoRoot
            $bakedSkill = Join-Path $commandSkill.Source "SKILL.md"
            if ($DryRun) {
                Write-Host "  [DRY RUN] Would write baked: $bakedSkill"
            } else {
                $utf8NoBom = New-Object System.Text.UTF8Encoding $false
                [System.IO.File]::WriteAllText($bakedSkill, $baked, $utf8NoBom)
                Write-Host "  [OK] $bakedSkill"
            }
        }
    }
}

# --- GAL command skill symlinks (commands/gal*/ → ~/.copilot/skills/gal*/ + ~/.agents/skills/gal*/) ---

Write-Host ""
Write-Host "=== GAL command skill symlinks ==="

if ($Uninstall) {
    foreach ($commandSkill in $commandSkillDirs) {
        Remove-SafeLink $commandSkill.CopilotTarget
        Remove-SafeLink $commandSkill.CodexTarget
    }
} else {
    foreach ($commandSkill in $commandSkillDirs) {
        New-SafeSymlink $commandSkill.CopilotTarget $commandSkill.Source "Directory" | Out-Null
        New-SafeSymlink $commandSkill.CodexTarget   $commandSkill.Source "Directory" | Out-Null
    }
}

# --- Migration: remove legacy gal-* dirs from installed locations ---

Write-Host ""
Write-Host "=== Migration: gal-* cleanup ==="

foreach ($skillsDir in @($skillsTarget, $geminiSkillsTarget, $codexSkillsTarget)) {
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
}
else {
    Write-Host "Setup complete: agents=$agentOk/$($agentFiles.Count), skills(copilot)=$skillOk/$($skillDirs.Count), skills(shared)=$codexSkillOk/$($skillDirs.Count), gal-root=$galRootOk/2"
    Write-Host "Note: If SKILL.template.md changes, re-run Setup-Machine.ps1 -Replace to regenerate."
    if ($agentFail -gt 0 -or $skillFail -gt 0 -or $codexSkillFail -gt 0) {
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
