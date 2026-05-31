#Requires -Version 5.1

<#!
.SYNOPSIS
    Core provider-neutral renderer for GAL.

.DESCRIPTION
    Builds the provider-neutral common package, validates it, and renders the
    superset canonical plugin root at ~/.gal/plugins/gal/ that serves all
    supported providers from one location (link-first).

    Outputs:
    - .claude-plugin/plugin.json  (Claude Code plugin manifest)
    - skills/                     (reusable skills)
    - commands/                   (flat command markdown files)
    - agents/<name>.md            (Claude-compatible filtered agent definitions)
    - agents/<name>.agent.md      (AGY-compatible unfiltered agent definitions)
    - .mcp.json                   (portable non-secret MCP server configuration)
    - plugin.json                 (AGY root manifest)
    - mcp_config.json             (AGY MCP configuration)
    - rules/gal.md                (AGY instruction corpus)

    Does NOT output:
    - user-scope Claude CLI MCP bridge state
    - provider install metadata
    - runtime-local secrets or machine-specific values

    When -Install is specified, projects to all AGY surfaces (link-first):
    - CLI junction:  ~/.gemini/antigravity-cli/plugins/gal  -> canonical root
    - IDE junction:  ~/.gemini/antigravity-ide/plugins/gal  -> canonical root
    - GUI-config:    agy plugin install <canonical root>     -> host-managed copy
    Also removes GAL-owned vestigial artifacts (whitelist-guarded).
#>

[CmdletBinding()]
param(
    [string]$RepoRoot,
    [string]$ResolvedPluginsFile,
    [switch]$Install,
    [switch]$Force
)

$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
    $RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
}

. (Join-Path (Join-Path $PSScriptRoot 'common') 'ProviderPlugin.ps1')
. (Join-Path (Join-Path $PSScriptRoot 'common') 'Common.ps1')

if (-not (Get-Variable -Scope Script -Name SetupContext -ErrorAction SilentlyContinue)) {
    $script:SetupContext = New-SetupContext -EntryScriptPath $MyInvocation.MyCommand.Path
}

# ---------------------------------------------------------------------------
# Helper: filter Claude agent frontmatter (keep only Claude-compatible keys)
# ---------------------------------------------------------------------------
function ConvertTo-ClaudePluginAgentContent {
    param([string]$SourcePath)

    $rawContent = Get-Content -LiteralPath $SourcePath -Raw -Encoding UTF8
    $normalizedContent = $rawContent -replace "`r`n", "`n"
    $lines = @($normalizedContent -split "`n")
    if ($lines.Count -lt 3 -or $lines[0] -ne '---') {
        return $rawContent
    }

    $closingIndex = -1
    for ($i = 1; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -eq '---') {
            $closingIndex = $i
            break
        }
    }

    if ($closingIndex -lt 1) {
        return $rawContent
    }

    $allowedKeys = @('name', 'description', 'model', 'effort', 'maxTurns', 'tools', 'disallowedTools', 'skills', 'memory', 'background', 'isolation')
    $frontmatterLines = [System.Collections.Generic.List[string]]::new()
    foreach ($line in $lines[1..($closingIndex - 1)]) {
        if ($line -notmatch '^([A-Za-z][A-Za-z0-9]*)\s*:') {
            continue
        }

        $key = $Matches[1]
        if ($allowedKeys -notcontains $key) {
            continue
        }

        if ($key -eq 'isolation') {
            $value = ($line -split ':', 2)[1].Trim()
            if ($value -notin @('worktree', '"worktree"', "'worktree'")) {
                continue
            }
        }

        $frontmatterLines.Add($line)
    }

    $bodyLines = if ($closingIndex + 1 -lt $lines.Count) { $lines[($closingIndex + 1)..($lines.Count - 1)] } else { @() }
    $resultLines = [System.Collections.Generic.List[string]]::new()
    $resultLines.Add('---')
    foreach ($line in $frontmatterLines) {
        $resultLines.Add($line)
    }
    $resultLines.Add('---')
    foreach ($line in $bodyLines) {
        $resultLines.Add($line)
    }

    return ($resultLines -join "`n")
}

# ---------------------------------------------------------------------------
# Helper: test Claude-portable MCP value (no ${} interpolation, no secrets)
# ---------------------------------------------------------------------------
function Test-ClaudePortableMcpValue {
    param([object]$Node)

    if ($null -eq $Node) { return $true }
    if ($Node -is [string]) {
        return ($Node -notmatch '\$\{[^}]+\}')
    }
    if ($Node -is [System.Collections.IDictionary]) {
        foreach ($key in $Node.Keys) {
            if (-not (Test-ClaudePortableMcpValue -Node $Node[$key])) {
                return $false
            }
        }
        return $true
    }
    if ($Node -is [System.Collections.IEnumerable] -and -not ($Node -is [string])) {
        foreach ($item in $Node) {
            if (-not (Test-ClaudePortableMcpValue -Node $item)) {
                return $false
            }
        }
        return $true
    }

    return $true
}

# ---------------------------------------------------------------------------
# Resolve plugins
# ---------------------------------------------------------------------------
$resolvedPlugins = $null
if (-not [string]::IsNullOrWhiteSpace($ResolvedPluginsFile)) {
    if (-not (Test-Path $ResolvedPluginsFile)) {
        throw "Resolved plugins file not found: $ResolvedPluginsFile"
    }

    $resolvedPlugins = Get-Content -LiteralPath $ResolvedPluginsFile -Raw | ConvertFrom-Json
}

# ---------------------------------------------------------------------------
# Build and validate common package
# ---------------------------------------------------------------------------
Write-Host 'Building provider-neutral package...' -ForegroundColor Cyan
$package = New-ProviderPluginPackage -RepoRoot $RepoRoot -ResolvedPlugins $resolvedPlugins

Write-Host 'Validating common package...' -ForegroundColor Cyan
$validation = Test-ProviderPluginPackage -Package $package
if (-not $validation.Valid) {
    Write-Host 'Validation failed:' -ForegroundColor Red
    foreach ($validationMessage in @($validation.Issues)) {
        Write-Host "  - $validationMessage" -ForegroundColor Red
    }
    throw 'Common package validation failed'
}
Write-Host 'Common package validated successfully.' -ForegroundColor Green

# ---------------------------------------------------------------------------
# Prepare superset canonical root
# ---------------------------------------------------------------------------
$artifactRoot = Get-GalPluginRoot -PluginId 'gal'
if (Test-Path $artifactRoot) {
    if (-not $Force) {
        throw "Artifact root already exists: $artifactRoot. Use -Force to overwrite."
    }
    Remove-Item -LiteralPath $artifactRoot -Recurse -Force
}

New-Item -ItemType Directory -Path $artifactRoot -Force | Out-Null

# ---------------------------------------------------------------------------
# CLAUDE: .claude-plugin/plugin.json
# ---------------------------------------------------------------------------
New-Item -ItemType Directory -Path (Join-Path $artifactRoot '.claude-plugin') -Force | Out-Null

Write-Host 'Rendering .claude-plugin/plugin.json...' -ForegroundColor Cyan
$pluginManifest = [ordered]@{
    '$schema' = 'https://json.schemastore.org/claude-code-plugin-manifest.json'
    name = 'gal'
    displayName = $package.metadata.displayName
    version = '1.0.0'
    description = 'Golem Agents Legion plugin for Claude Code'
    author = [ordered]@{
        name = 'GAL'
    }
    homepage = 'https://github.com/leetz/Golem-Agents-Legion'
    repository = 'https://github.com/leetz/Golem-Agents-Legion'
    license = 'MIT'
    keywords = @('gal', 'golem-agents-legion', 'claude-code', 'plugin')
}
Write-JsonOrderedMap (Get-ClaudePluginManifestPath -PluginRoot $artifactRoot) $pluginManifest
Write-Host '  -> .claude-plugin/plugin.json' -ForegroundColor Gray

# ---------------------------------------------------------------------------
# SHARED: skills/
# ---------------------------------------------------------------------------
Write-Host 'Rendering skills...' -ForegroundColor Cyan
$skillsDir = Join-Path $artifactRoot 'skills'
New-Item -ItemType Directory -Path $skillsDir -Force | Out-Null
foreach ($skill in $package.skills) {
    $destDir = Join-Path $skillsDir $skill.name
    New-Item -ItemType Directory -Path $destDir -Force | Out-Null
    Copy-Item -LiteralPath $skill.sourcePath -Destination (Join-Path $destDir 'SKILL.md') -Force
    Write-Host "  -> skills/$($skill.name)/SKILL.md" -ForegroundColor Gray
}

# ---------------------------------------------------------------------------
# CLAUDE: commands/
# ---------------------------------------------------------------------------
Write-Host 'Rendering commands...' -ForegroundColor Cyan
$commandsDir = Join-Path $artifactRoot 'commands'
New-Item -ItemType Directory -Path $commandsDir -Force | Out-Null
foreach ($commandSkill in $package.commandSkills) {
    $destFile = Join-Path $commandsDir "$($commandSkill.name).md"
    Copy-Item -LiteralPath $commandSkill.sourcePath -Destination $destFile -Force
    Write-Host "  -> commands/$($commandSkill.name).md" -ForegroundColor Gray
}

# ---------------------------------------------------------------------------
# SHARED: agents/  — two formats for superset compatibility
#   .md        = Claude-filtered (Claude Code consumes this)
#   .agent.md  = unfiltered copy (AGY consumes this)
# ---------------------------------------------------------------------------
Write-Host 'Rendering agents...' -ForegroundColor Cyan
$agentsDir = Join-Path $artifactRoot 'agents'
New-Item -ItemType Directory -Path $agentsDir -Force | Out-Null
foreach ($agent in $package.agents) {
    $agentName = $agent.name -replace '\.agent$', ''

    # Claude format: filtered frontmatter, .md extension
    $claudeDestFile = Join-Path $agentsDir "$agentName.md"
    $agentContent = ConvertTo-ClaudePluginAgentContent -SourcePath $agent.sourcePath
    Set-Content -LiteralPath $claudeDestFile -Value $agentContent -Encoding UTF8
    Write-Host "  -> agents/$agentName.md (Claude)" -ForegroundColor Gray

    # AGY format: unfiltered copy, .agent.md extension
    $agyDestFile = Join-Path $agentsDir "$agentName.agent.md"
    Copy-Item -LiteralPath $agent.sourcePath -Destination $agyDestFile -Force
    Write-Host "  -> agents/$agentName.agent.md (AGY)" -ForegroundColor Gray
}

# ---------------------------------------------------------------------------
# CLAUDE: .mcp.json  (portable MCP, no secrets/headers/env)
# ---------------------------------------------------------------------------
if ($package.mcpSpec -and $package.mcpSpec.canonicalSource) {
    Write-Host 'Rendering .mcp.json...' -ForegroundColor Cyan
    $mcpManifest = Read-JsonOrderedMap $package.mcpSpec.canonicalSource
    if ($null -ne $mcpManifest -and $mcpManifest.Contains('servers') -and $mcpManifest['servers'] -is [System.Collections.IDictionary]) {
        $portableServers = [ordered]@{}
        foreach ($serverName in $mcpManifest['servers'].Keys) {
            $serverConfig = $mcpManifest['servers'][$serverName]
            if ($serverConfig -isnot [System.Collections.IDictionary]) {
                continue
            }
            if ($serverConfig.Contains('headers') -or $serverConfig.Contains('env')) {
                continue
            }
            if (-not (Test-ClaudePortableMcpValue -Node $serverConfig)) {
                continue
            }

            $converted = [ordered]@{}
            foreach ($key in $serverConfig.Keys) {
                if ($key -eq 'type') { continue }
                $converted[$key] = $serverConfig[$key]
            }
            if ($converted.Count -gt 0) {
                $portableServers[$serverName] = $converted
            }
        }

        $claudeMcp = [ordered]@{
            mcpServers = $portableServers
        }
        Write-JsonOrderedMap (Join-Path $artifactRoot '.mcp.json') $claudeMcp
        Write-Host '  -> .mcp.json' -ForegroundColor Gray
    }
}

# ---------------------------------------------------------------------------
# AGY: root plugin.json
# ---------------------------------------------------------------------------
Write-Host 'Rendering plugin.json (AGY root manifest)...' -ForegroundColor Cyan
$agyPluginJson = [ordered]@{
    name = 'gal'
    displayName = $package.metadata.displayName
    version = '1.0.0'
    generatedAt = $package.metadata.generatedAt
    description = 'Golem Agents Legion plugin for AGY CLI'
    canonicalPackage = [ordered]@{
        packageId = $package.packageSchema.packageId
        schemaId = $package.packageSchema.schemaId
        canonicalProvider = $package.packageSchema.canonicalProvider
        sourcePlugins = $package.sourcePlugins
    }
    deferredCompanionPlugins = $package.deferredCompanionPlugins
    skills = [System.Collections.Generic.List[object]]::new()
    agents = [System.Collections.Generic.List[object]]::new()
    hasMcp = ($null -ne $package.mcpSpec)
    hasInstructions = ($package.instructionCorpus.sources.Count -gt 0)
    skippedComponents = $package.skippedComponents
}

# Skill entries (reusable)
foreach ($skill in $package.skills) {
    $agyPluginJson.skills.Add([ordered]@{
        name = $skill.name
        type = 'skill'
        source = "skills/$($skill.name)/SKILL.md"
    })
}

# Command skill entries (rendered as skills in AGY)
foreach ($cmdSkill in $package.commandSkills) {
    $agyPluginJson.skills.Add([ordered]@{
        name = $cmdSkill.name
        type = 'command-skill'
        source = "skills/$($cmdSkill.name)/SKILL.md"
    })
}

# Agent entries (reference .agent.md format)
foreach ($agent in $package.agents) {
    $agentBaseName = $agent.name -replace '\.agent$', ''
    $agyPluginJson.agents.Add([ordered]@{
        name = $agentBaseName
        source = "agents/$($agentBaseName).agent.md"
    })
}

$agyPluginJson | ConvertTo-Json -Depth 10 | Set-Content -Path (Join-Path $artifactRoot 'plugin.json') -Encoding UTF8
Write-Host '  -> plugin.json' -ForegroundColor Gray

# ---------------------------------------------------------------------------
# AGY: mcp_config.json  (AGY MCP format: url -> serverUrl, remove type)
# ---------------------------------------------------------------------------
if ($package.mcpSpec -and $package.mcpSpec.canonicalSource) {
    Write-Host 'Rendering mcp_config.json (AGY MCP)...' -ForegroundColor Cyan
    $mcpRaw = Read-JsonOrderedMap $package.mcpSpec.canonicalSource
    if ($null -ne $mcpRaw) {
        # Merge local overrides if present
        if ($package.mcpSpec.hasLocalOverrides) {
            $mcpLocalFile = Join-Path (Split-Path $package.mcpSpec.canonicalSource -Parent) 'mcp.local.json'
            if (Test-Path $mcpLocalFile) {
                $localManifest = Read-JsonOrderedMap $mcpLocalFile
                if ($null -ne $localManifest) {
                    $mcpRaw = Merge-OrderedMap $mcpRaw $localManifest
                }
            }
        }

        $agyMcpConfig = [ordered]@{}
        if ($mcpRaw.Contains('servers') -and $mcpRaw['servers'] -is [System.Collections.IDictionary]) {
            $agyServers = [ordered]@{}
            foreach ($serverName in $mcpRaw['servers'].Keys) {
                $serverConfig = $mcpRaw['servers'][$serverName]
                if ($serverConfig -is [System.Collections.IDictionary]) {
                    $converted = [ordered]@{}
                    foreach ($key in $serverConfig.Keys) {
                        if ($key -eq 'type') { continue }
                        if ($key -eq 'url') {
                            $converted['serverUrl'] = [string]$serverConfig['url']
                            continue
                        }
                        $converted[$key] = $serverConfig[$key]
                    }
                    $agyServers[$serverName] = $converted
                }
                else {
                    $agyServers[$serverName] = $serverConfig
                }
            }
            $agyMcpConfig['mcpServers'] = $agyServers
        }
        if ($mcpRaw.Contains('inputs')) {
            $agyMcpConfig['inputs'] = $mcpRaw['inputs']
        }

        Write-JsonOrderedMap (Join-Path $artifactRoot 'mcp_config.json') $agyMcpConfig
        Write-Host '  -> mcp_config.json' -ForegroundColor Gray
    }
}

# ---------------------------------------------------------------------------
# AGY: rules/gal.md  (instruction corpus)
# ---------------------------------------------------------------------------
if ($package.instructionCorpus.sources.Count -gt 0) {
    Write-Host 'Rendering rules/gal.md...' -ForegroundColor Cyan
    $rulesDir = Join-Path $artifactRoot 'rules'
    New-Item -ItemType Directory -Path $rulesDir -Force | Out-Null

    $corpusLines = [System.Collections.Generic.List[string]]::new()
    $corpusLines.Add("# GAL Instruction Corpus")
    $corpusLines.Add("")
    $corpusLines.Add("> Generated by Build-CorePlugin.ps1")
    $corpusLines.Add("> DO NOT EDIT DIRECTLY — regenerate from source contracts")
    $corpusLines.Add("")

    foreach ($source in $package.instructionCorpus.sources) {
        $relPath = $source.Substring($RepoRoot.Length + 1)
        $corpusLines.Add("## Source: $relPath")
        $corpusLines.Add("")
        $content = Get-Content -Raw -Path $source
        $corpusLines.Add($content)
        $corpusLines.Add("")
        $corpusLines.Add("---")
        $corpusLines.Add("")
    }

    $rulesFile = Join-Path $rulesDir 'gal.md'
    $corpusLines -join "`n" | Set-Content -Path $rulesFile -Encoding UTF8
    Write-Host '  -> rules/gal.md' -ForegroundColor Gray
}

# ---------------------------------------------------------------------------
# -Install: project to all AGY surfaces (link-first) + cleanup
# ---------------------------------------------------------------------------
if ($Install) {
    Write-Host '' -ForegroundColor Cyan
    Write-Host '=== AGY Surface Projection (link-first) ===' -ForegroundColor Cyan

    # Validate superset root is AGY-compatible before touching any surfaces
    if (Test-CommandAvailable 'agy') {
        Write-Host 'Validating superset canonical root with agy...' -ForegroundColor Cyan
        $validateOutput = (& agy plugin validate $artifactRoot 2>&1 | Out-String)
        if ($LASTEXITCODE -ne 0) {
            Write-Host ("  [WARN] agy plugin validate returned non-zero; continuing with install:`n{0}" -f $validateOutput.Trim()) -ForegroundColor Yellow
        }
        else {
            Write-Host '  [OK] agy plugin validate passed on canonical root.' -ForegroundColor Green
        }
    }

    # Surface 1: CLI junction — ~/.gemini/antigravity-cli/plugins/gal -> canonical root (link-first)
    $cliTarget = Get-AgyPluginInstallTarget  # ~/.gemini/antigravity-cli/plugins/gal
    Write-Host "Projecting CLI surface: $cliTarget -> $artifactRoot" -ForegroundColor Cyan
    if (Test-Path $cliTarget) {
        Remove-Item -LiteralPath $cliTarget -Recurse -Force
    }
    $cliParent = Split-Path $cliTarget -Parent
    if (-not (Test-Path $cliParent)) {
        New-Item -ItemType Directory -Path $cliParent -Force | Out-Null
    }
    if (-not (New-SafeSymlink -LinkPath $cliTarget -TargetPath $artifactRoot -Type 'Directory')) {
        throw "Failed to project AGY CLI plugin into $cliTarget"
    }
    Write-Host "  [OK] CLI junction: $cliTarget" -ForegroundColor Green

    # Surface 2: IDE junction — ~/.gemini/antigravity-ide/plugins/gal -> canonical root (link-first)
    $idePluginsDir = Join-Path $env:USERPROFILE '.gemini\antigravity-ide\plugins'
    $ideTarget = Join-Path $idePluginsDir 'gal'
    Write-Host "Projecting IDE surface: $ideTarget -> $artifactRoot" -ForegroundColor Cyan
    if (Test-Path $ideTarget) {
        Remove-Item -LiteralPath $ideTarget -Recurse -Force
    }
    if (-not (Test-Path $idePluginsDir)) {
        New-Item -ItemType Directory -Path $idePluginsDir -Force | Out-Null
    }
    if (-not (New-SafeSymlink -LinkPath $ideTarget -TargetPath $artifactRoot -Type 'Directory')) {
        # IDE may not support junctions; fall back to robocopy/copy
        Write-Host '  [FALLBACK] Junction not supported for IDE surface; copying...' -ForegroundColor Yellow
        Copy-Item -LiteralPath $artifactRoot -Destination $ideTarget -Recurse -Force
    }
    Write-Host "  [OK] IDE surface: $ideTarget" -ForegroundColor Green

    # Surface 3: GUI-config — agy plugin install (host-managed copy from canonical root)
    if (Test-CommandAvailable 'agy') {
        Write-Host 'Installing into shared AGY config store via `agy plugin install`...' -ForegroundColor Cyan
        $agyInstallOutput = (& agy plugin install $artifactRoot 2>&1 | Out-String)
        if ($LASTEXITCODE -eq 0) {
            Write-Host '  [OK] agy plugin install succeeded (GUI-config surface).' -ForegroundColor Green
        }
        else {
            Write-Host ("  [WARN] agy plugin install returned non-zero; check output:`n{0}" -f $agyInstallOutput.Trim()) -ForegroundColor Yellow
        }
    }
    else {
        Write-Host '  [SKIP] agy CLI not on PATH; skipping GUI-config store install.' -ForegroundColor Yellow
    }

    # Cleanup: remove GAL-owned vestigial artifacts (whitelist only — do NOT touch ~/.antigravity* IDE dirs)
    Write-Host '' -ForegroundColor Cyan
    Write-Host '=== R-CLEANUP: GAL-owned vestigial artifacts ===' -ForegroundColor Cyan

    # Vestigial claude dist (marketplace already uses canonical root)
    $claudeDist = Join-Path $env:USERPROFILE '.gal\dist\provider-plugins\claude'
    if (Test-Path $claudeDist) {
        Remove-Item -LiteralPath $claudeDist -Recurse -Force
        Write-Host "  [REMOVED] $claudeDist (vestigial claude dist)" -ForegroundColor Green
    }
    else {
        Write-Host "  [SKIP] $claudeDist not found (already clean)" -ForegroundColor Gray
    }

    # Empty ~/.antigravitycli shell directory
    $antigravityCli = Join-Path $env:USERPROFILE '.antigravitycli'
    if (Test-Path $antigravityCli) {
        $contents = Get-ChildItem -LiteralPath $antigravityCli -Force -ErrorAction SilentlyContinue
        if ($null -eq $contents -or $contents.Count -eq 0) {
            Remove-Item -LiteralPath $antigravityCli -Force
            Write-Host "  [REMOVED] $antigravityCli (empty shell directory)" -ForegroundColor Green
        }
        else {
            Write-Host "  [SKIP] $antigravityCli is not empty; leaving intact" -ForegroundColor Yellow
        }
    }
    else {
        Write-Host "  [SKIP] $antigravityCli not found (already clean)" -ForegroundColor Gray
    }

    # Conditional: AGY dist — only remove when CLI junction has been confirmed to point to canonical root
    $agyDist = Join-Path $env:USERPROFILE '.gal\dist\provider-plugins\agy'
    if (Test-Path $agyDist) {
        # Check CLI junction target: if it points to canonical root (not dist), dist is orphaned
        $cliJunction = Get-Item -LiteralPath $cliTarget -ErrorAction SilentlyContinue
        $cliIsLinkedToCanonical = ($null -ne $cliJunction -and $cliJunction.LinkType -ne '' -and
                                   $cliJunction.Target -and
                                   ($cliJunction.Target -replace '\\', '/').TrimEnd('/') -eq
                                   ($artifactRoot -replace '\\', '/').TrimEnd('/'))
        if ($cliIsLinkedToCanonical) {
            Remove-Item -LiteralPath $agyDist -Recurse -Force
            Write-Host "  [REMOVED] $agyDist (link-first convergence confirmed; dist orphaned)" -ForegroundColor Green
        }
        else {
            Write-Host "  [RETAIN] $agyDist (CLI junction not yet confirmed to target canonical; retaining)" -ForegroundColor Yellow
        }
    }
}

Write-Host ''
Write-Host 'Core plugin artifact rendered successfully.' -ForegroundColor Green
Write-Host "Artifact root: $artifactRoot" -ForegroundColor Green
