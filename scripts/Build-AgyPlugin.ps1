#Requires -Version 5.1

<#
.SYNOPSIS
    AGY plugin renderer for GAL.

.DESCRIPTION
    Builds the provider-neutral common package, validates it, and renders
    AGY-specific artifacts to ~/.gal/dist/provider-plugins/agy/gal/.

    Outputs:
    - plugin.json          (manifest with stable name: gal)
    - skills/              (reusable skills + command skills)
    - agents/              (agent definitions)
    - rules/gal.md         (instruction corpus)
    - mcp_config.json      (MCP server configuration)

    Does NOT output:
    - hooks.json
    - scripts/
    - marketplace metadata
    - provider stubs
#>

[CmdletBinding()]
param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')),
    [string]$ResolvedPluginsFile,
    [switch]$Install,
    [switch]$Force
)

$ErrorActionPreference = 'Stop'

# --- Import common helpers ---
$providerPluginScript = Join-Path $PSScriptRoot 'common' 'ProviderPlugin.ps1'
if (-not (Test-Path $providerPluginScript)) {
    throw "Common helpers not found: $providerPluginScript"
}
. $providerPluginScript

$commonHelpersScript = Join-Path $PSScriptRoot 'common' 'Common.ps1'
if (-not (Test-Path $commonHelpersScript)) {
    throw "Common helpers not found: $commonHelpersScript"
}
. $commonHelpersScript

if (-not (Get-Variable -Scope Script -Name SetupContext -ErrorAction SilentlyContinue)) {
    $script:SetupContext = New-SetupContext -EntryScriptPath $MyInvocation.MyCommand.Path
}

$resolvedPlugins = $null
if (-not [string]::IsNullOrWhiteSpace($ResolvedPluginsFile)) {
    if (-not (Test-Path $ResolvedPluginsFile)) {
        throw "Resolved plugins file not found: $ResolvedPluginsFile"
    }

    $resolvedPlugins = Get-Content -LiteralPath $ResolvedPluginsFile -Raw | ConvertFrom-Json
}

# --- Build and validate common package ---
Write-Host "Building provider-neutral package..." -ForegroundColor Cyan
$package = New-ProviderPluginPackage -RepoRoot $RepoRoot -ResolvedPlugins $resolvedPlugins

Write-Host "Validating common package..." -ForegroundColor Cyan
$validation = Test-ProviderPluginPackage -Package $package
if (-not $validation.Valid) {
    Write-Host "Validation failed:" -ForegroundColor Red
    $validationIssues = @($validation.Issues)
    foreach ($validationMessage in $validationIssues) {
        Write-Host "  - $validationMessage" -ForegroundColor Red
    }
    throw "Common package validation failed"
}
Write-Host "Common package validated successfully." -ForegroundColor Green

# --- Prepare artifact root ---
$artifactRoot = Get-AgyPluginArtifactRoot -RepoRoot $RepoRoot
if (Test-Path $artifactRoot) {
    if (-not $Force) {
        throw "Artifact root already exists: $artifactRoot. Use -Force to overwrite."
    }
    Remove-Item -LiteralPath $artifactRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $artifactRoot -Force | Out-Null

# --- Render plugin.json ---
Write-Host "Rendering plugin.json..." -ForegroundColor Cyan
$pluginJson = [ordered]@{
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

# Add skill entries
foreach ($skill in $package.skills) {
    $pluginJson.skills.Add([ordered]@{
        name = $skill.name
        type = 'skill'
        source = "skills/$($skill.name)/SKILL.md"
    })
}

# Add command skill entries (rendered as skills in AGY)
foreach ($cmdSkill in $package.commandSkills) {
    $pluginJson.skills.Add([ordered]@{
        name = $cmdSkill.name
        type = 'command-skill'
        source = "skills/$($cmdSkill.name)/SKILL.md"
    })
}

# Add agent entries
foreach ($agent in $package.agents) {
    $agentBaseName = $agent.name -replace '\.agent$', ''
    $pluginJson.agents.Add([ordered]@{
        name = $agentBaseName
        source = "agents/$($agentBaseName).agent.md"
    })
}

$pluginJsonPath = Join-Path $artifactRoot 'plugin.json'
$pluginJson | ConvertTo-Json -Depth 10 | Set-Content -Path $pluginJsonPath -Encoding UTF8
Write-Host "  -> $pluginJsonPath" -ForegroundColor Gray

# --- Render skills ---
Write-Host "Rendering skills..." -ForegroundColor Cyan
$skillsDir = Join-Path $artifactRoot 'skills'
New-Item -ItemType Directory -Path $skillsDir -Force | Out-Null

# Render reusable skills
foreach ($skill in $package.skills) {
    $destDir = Join-Path $skillsDir $skill.name
    New-Item -ItemType Directory -Path $destDir -Force | Out-Null
    $destFile = Join-Path $destDir 'SKILL.md'
    Copy-Item -LiteralPath $skill.sourcePath -Destination $destFile -Force
    Write-Host "  -> skills/$($skill.name)/SKILL.md" -ForegroundColor Gray
}

# Render command skills (as skills in AGY)
foreach ($cmdSkill in $package.commandSkills) {
    $destDir = Join-Path $skillsDir $cmdSkill.name
    New-Item -ItemType Directory -Path $destDir -Force | Out-Null
    $destFile = Join-Path $destDir 'SKILL.md'
    Copy-Item -LiteralPath $cmdSkill.sourcePath -Destination $destFile -Force
    Write-Host "  -> skills/$($cmdSkill.name)/SKILL.md (command-skill)" -ForegroundColor Gray
}

# --- Render agents ---
Write-Host "Rendering agents..." -ForegroundColor Cyan
$agentsDir = Join-Path $artifactRoot 'agents'
New-Item -ItemType Directory -Path $agentsDir -Force | Out-Null

foreach ($agent in $package.agents) {
    $agentBaseName = $agent.name -replace '\.agent$', ''
    $destFile = Join-Path $agentsDir "$($agentBaseName).agent.md"
    Copy-Item -LiteralPath $agent.sourcePath -Destination $destFile -Force
    Write-Host "  -> agents/$($agentBaseName).agent.md" -ForegroundColor Gray
}

# --- Render MCP config ---
if ($package.mcpSpec -and $package.mcpSpec.canonicalSource) {
    Write-Host "Rendering mcp_config.json..." -ForegroundColor Cyan

    # Load canonical MCP spec
    $mcpManifest = Read-JsonOrderedMap $package.mcpSpec.canonicalSource
    if ($null -ne $mcpManifest) {
        # Merge local overrides if present (boundary only; no resolved values in common model)
        if ($package.mcpSpec.hasLocalOverrides) {
            $mcpLocalFile = Join-Path (Split-Path $package.mcpSpec.canonicalSource -Parent) 'mcp.local.json'
            if (Test-Path $mcpLocalFile) {
                $localManifest = Read-JsonOrderedMap $mcpLocalFile
                if ($null -ne $localManifest) {
                    $mcpManifest = Merge-OrderedMap $mcpManifest $localManifest
                }
            }
        }

        # Convert to AGY format: url -> serverUrl, remove type field
        $agyMcpConfig = [ordered]@{}
        if ($mcpManifest.Contains('servers') -and $mcpManifest['servers'] -is [System.Collections.IDictionary]) {
            $agyServers = [ordered]@{}
            foreach ($serverName in $mcpManifest['servers'].Keys) {
                $serverConfig = $mcpManifest['servers'][$serverName]
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

        # Preserve inputs if present (AGY supports inputs)
        if ($mcpManifest.Contains('inputs')) {
            $agyMcpConfig['inputs'] = $mcpManifest['inputs']
        }

        $mcpDest = Join-Path $artifactRoot 'mcp_config.json'
        Write-JsonOrderedMap $mcpDest $agyMcpConfig
        Write-Host "  -> mcp_config.json" -ForegroundColor Gray
    }
    else {
        Write-Host "  [WARN] Could not parse MCP source file; skipping mcp_config.json" -ForegroundColor Yellow
    }
}

# --- Render rules/gal.md (instruction corpus) ---
if ($package.instructionCorpus.sources.Count -gt 0) {
    Write-Host "Rendering rules/gal.md..." -ForegroundColor Cyan
    $rulesDir = Join-Path $artifactRoot 'rules'
    New-Item -ItemType Directory -Path $rulesDir -Force | Out-Null

    $corpusLines = [System.Collections.Generic.List[string]]::new()
    $corpusLines.Add("# GAL Instruction Corpus")
    $corpusLines.Add("")
    $corpusLines.Add("> Generated by Build-AgyPlugin.ps1")
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
    Write-Host "  -> rules/gal.md" -ForegroundColor Gray
}

# --- Install if requested ---
if ($Install) {
    $installTarget = Get-AgyPluginInstallTarget
    Write-Host "Installing to $installTarget..." -ForegroundColor Cyan
    if (Test-Path $installTarget) {
        Remove-Item -LiteralPath $installTarget -Recurse -Force
    }
    $installParent = Split-Path $installTarget -Parent
    if (-not (Test-Path $installParent)) {
        New-Item -ItemType Directory -Path $installParent -Force | Out-Null
    }
    Copy-Item -LiteralPath $artifactRoot -Destination $installTarget -Recurse -Force
    Write-Host "Installed to $installTarget" -ForegroundColor Green
}

Write-Host "`nAGY plugin rendered successfully to: $artifactRoot" -ForegroundColor Green
Write-Host "Skills: $($package.skills.Count + $package.commandSkills.Count)" -ForegroundColor White
Write-Host "Agents: $($package.agents.Count)" -ForegroundColor White
Write-Host "MCP: $(if ($package.mcpSpec) { 'yes' } else { 'no' })" -ForegroundColor White
Write-Host "Instructions: $($package.instructionCorpus.sources.Count) sources" -ForegroundColor White
Write-Host "Deferred companions: $($package.deferredCompanionPlugins.Count)" -ForegroundColor White
