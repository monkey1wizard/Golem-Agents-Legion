#Requires -Version 5.1

<#!
.SYNOPSIS
    Claude plugin renderer for GAL.

.DESCRIPTION
    Builds the provider-neutral common package, validates it, and renders
    Claude-specific artifacts to the canonical plugin root under ~/.gal/plugins/gal/.

    Outputs:
    - .claude-plugin/plugin.json  (Claude plugin manifest)
    - skills/                     (reusable skills)
    - commands/                   (flat command markdown files)
    - agents/                     (Claude-compatible agent definitions)
    - .mcp.json                   (portable non-secret MCP server configuration)

    Does NOT output:
    - user-scope Claude CLI MCP bridge state
    - provider install metadata
    - runtime-local secrets or machine-specific values
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

$resolvedPlugins = $null
if (-not [string]::IsNullOrWhiteSpace($ResolvedPluginsFile)) {
    if (-not (Test-Path $ResolvedPluginsFile)) {
        throw "Resolved plugins file not found: $ResolvedPluginsFile"
    }

    $resolvedPlugins = Get-Content -LiteralPath $ResolvedPluginsFile -Raw | ConvertFrom-Json
}

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

$artifactRoot = Get-GalPluginRoot -PluginId 'gal'
if (Test-Path $artifactRoot) {
    if (-not $Force) {
        throw "Artifact root already exists: $artifactRoot. Use -Force to overwrite."
    }
    Remove-Item -LiteralPath $artifactRoot -Recurse -Force
}

New-Item -ItemType Directory -Path $artifactRoot -Force | Out-Null
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

Write-Host 'Rendering skills...' -ForegroundColor Cyan
$skillsDir = Join-Path $artifactRoot 'skills'
New-Item -ItemType Directory -Path $skillsDir -Force | Out-Null
foreach ($skill in $package.skills) {
    $destDir = Join-Path $skillsDir $skill.name
    New-Item -ItemType Directory -Path $destDir -Force | Out-Null
    Copy-Item -LiteralPath $skill.sourcePath -Destination (Join-Path $destDir 'SKILL.md') -Force
    Write-Host "  -> skills/$($skill.name)/SKILL.md" -ForegroundColor Gray
}

Write-Host 'Rendering commands...' -ForegroundColor Cyan
$commandsDir = Join-Path $artifactRoot 'commands'
New-Item -ItemType Directory -Path $commandsDir -Force | Out-Null
foreach ($commandSkill in $package.commandSkills) {
    $destFile = Join-Path $commandsDir "$($commandSkill.name).md"
    Copy-Item -LiteralPath $commandSkill.sourcePath -Destination $destFile -Force
    Write-Host "  -> commands/$($commandSkill.name).md" -ForegroundColor Gray
}

Write-Host 'Rendering agents...' -ForegroundColor Cyan
$agentsDir = Join-Path $artifactRoot 'agents'
New-Item -ItemType Directory -Path $agentsDir -Force | Out-Null
foreach ($agent in $package.agents) {
    $agentName = $agent.name -replace '\.agent$', ''
    $destFile = Join-Path $agentsDir "$agentName.md"
    $agentContent = ConvertTo-ClaudePluginAgentContent -SourcePath $agent.sourcePath
    Set-Content -LiteralPath $destFile -Value $agentContent -Encoding UTF8
    Write-Host "  -> agents/$agentName.md" -ForegroundColor Gray
}

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

if ($Install) {
    Write-Host '  [INFO] Renderer completed; install orchestration performs Claude lifecycle validation and session-load capability checks.' -ForegroundColor Yellow
}

Write-Host ''
Write-Host 'Claude plugin artifact rendered successfully.' -ForegroundColor Green
Write-Host "Artifact root: $artifactRoot" -ForegroundColor Green
