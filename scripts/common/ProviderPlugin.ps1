#Requires -Version 5.1

<#
.SYNOPSIS
    Provider-neutral plugin package model helpers for GAL.

.DESCRIPTION
    Builds and validates the provider-neutral package model that represents
    the minimum shared substrate across AGY CLI, Copilot CLI, Codex, and Claude Code.
    The common model contains no provider-specific output paths, no resolved local
    secrets, and no runtimeScripts.
#>

function Get-GalCoreCanonicalPackageSchema {
    <#
    .SYNOPSIS
        Returns the Claude-compatible canonical package schema for gal-core.
    #>

    return [ordered]@{
        schemaId = 'claude-compatible-gal-core-v1'
        schemaVersion = 1
        packageId = 'gal-core'
        packageKind = 'canonical-plugin'
        canonicalProvider = 'claude'
        compatibleProviders = @('claude', 'copilot', 'codex', 'agy')
        componentRoots = [ordered]@{
            skills = 'skills'
            commands = 'commands'
            agents = 'agents'
            mcp = 'provider-managed'
            lsp = 'provider-managed'
        }
        nativeInstallProviders = @('claude', 'copilot', 'codex')
        managedShortcutProviders = @('agy')
    }
}

function Get-GalCoreCopiedCompanionSkillPatterns {
    <#
    .SYNOPSIS
        Returns copied skill prefixes that belong to companion plugins rather than gal-core.
    #>

    return @('dart-*', 'flutter-*')
}

function Resolve-CanonicalPackageInput {
    <#
    .SYNOPSIS
        Partitions resolver output into gal-core source input and deferred companion plugins.
    #>
    param(
        [array]$ResolvedPlugins
    )

    $sourcePlugins = [System.Collections.Generic.List[object]]::new()
    $deferredCompanionPlugins = [System.Collections.Generic.List[object]]::new()

    if ($ResolvedPlugins -and $ResolvedPlugins.Count -gt 0) {
        foreach ($plugin in $ResolvedPlugins) {
            if ($plugin.pluginId -eq 'gal-core' -or $plugin.supportTier -eq 'official-gal') {
                $sourcePlugins.Add([ordered]@{
                    pluginId = $plugin.pluginId
                    supportTier = $plugin.supportTier
                    sourceType = $plugin.sourceType
                })
                continue
            }

            $deferredCompanionPlugins.Add([ordered]@{
                pluginId = $plugin.pluginId
                supportTier = $plugin.supportTier
                sourceType = $plugin.sourceType
            })
        }
    }
    else {
        $sourcePlugins.Add([ordered]@{
            pluginId = 'gal-core'
            supportTier = 'official-gal'
            sourceType = 'official-gal'
        })
    }

    if (($sourcePlugins | Where-Object { $_.pluginId -eq 'gal-core' }).Count -eq 0) {
        throw 'Canonical package input must include gal-core when resolver output is provided.'
    }

    return [pscustomobject]@{
        SourcePlugins = @($sourcePlugins)
        DeferredCompanionPlugins = @($deferredCompanionPlugins)
    }
}

function New-ProviderPluginPackage {
    <#
    .SYNOPSIS
        Builds a provider-neutral plugin package from the GAL repo source contracts.
    #>
    param(
        [string]$RepoRoot,
        [array]$ResolvedPlugins
    )

    $packageInput = Resolve-CanonicalPackageInput -ResolvedPlugins $ResolvedPlugins
    $excludedSkillPatterns = Get-GalCoreCopiedCompanionSkillPatterns

    $package = [ordered]@{
        packageSchema = Get-GalCoreCanonicalPackageSchema
        metadata = [ordered]@{
            name = 'gal'
            displayName = 'Golem Agents Legion'
            generatedAt = (Get-Date -Format 'o')
        }
        sourcePlugins = @($packageInput.SourcePlugins)
        deferredCompanionPlugins = @($packageInput.DeferredCompanionPlugins)
        skills = [System.Collections.Generic.List[object]]::new()
        commandSkills = [System.Collections.Generic.List[object]]::new()
        mcpSpec = $null
        instructionCorpus = [ordered]@{
            sources = [System.Collections.Generic.List[string]]::new()
        }
        agents = [System.Collections.Generic.List[object]]::new()
        skippedComponents = [System.Collections.Generic.List[string]]::new()
    }

    # --- Reusable skills ---
    $skillsDir = Join-Path $RepoRoot 'skills'
    if (Test-Path $skillsDir) {
        foreach ($skillDir in Get-ChildItem $skillsDir -Directory | Sort-Object Name) {
            $isDeferredCompanionSkill = $false
            foreach ($pattern in $excludedSkillPatterns) {
                if ($skillDir.Name -like $pattern) {
                    $isDeferredCompanionSkill = $true
                    break
                }
            }
            if ($isDeferredCompanionSkill) {
                continue
            }

            $skillFile = Join-Path $skillDir.FullName 'SKILL.md'
            if (Test-Path $skillFile) {
                $package.skills.Add([ordered]@{
                    name = $skillDir.Name
                    sourcePath = $skillFile
                })
            }
        }
    }

    # --- Command skills (rendered as skills in the common model) ---
    $commandsDir = Join-Path $RepoRoot 'commands'
    if (Test-Path $commandsDir) {
        foreach ($cmdDir in Get-ChildItem $commandsDir -Directory | Sort-Object Name) {
            $skillFile = Join-Path $cmdDir.FullName 'SKILL.md'
            $templateFile = Join-Path $cmdDir.FullName 'SKILL.template.md'
            $sourceFile = if (Test-Path $skillFile) { $skillFile } elseif (Test-Path $templateFile) { $templateFile } else { $null }
            if ($sourceFile) {
                $package.commandSkills.Add([ordered]@{
                    name = $cmdDir.Name
                    sourcePath = $sourceFile
                })
            }
        }
    }

    # --- MCP spec (canonical only; local overrides flagged but not resolved) ---
    $mcpFile = Join-Path $RepoRoot 'mcp.json'
    $mcpLocalFile = Join-Path $RepoRoot 'mcp.local.json'
    if (Test-Path $mcpFile) {
        $package.mcpSpec = [ordered]@{
            canonicalSource = $mcpFile
            hasLocalOverrides = Test-Path $mcpLocalFile
        }
    }

    # --- Instruction corpus sources ---
    $corpusSources = @(
        Join-Path $RepoRoot '.dev/project.md'
        Join-Path $RepoRoot 'conventions/conventions.md'
        Join-Path $RepoRoot 'conventions/token-budget.md'
        Join-Path $RepoRoot 'workflows/coding.md'
        Join-Path $RepoRoot 'model-roles.md'
    ) | Where-Object { Test-Path $_ }
    foreach ($src in $corpusSources) {
        $package.instructionCorpus.sources.Add($src)
    }

    # --- Agents (optional payload; Codex explicitly skipped at renderer level) ---
    $agentsDir = Join-Path $RepoRoot 'agent'
    if (Test-Path $agentsDir) {
        foreach ($agentFile in Get-ChildItem $agentsDir -Filter '*.agent.md' | Sort-Object Name) {
            $package.agents.Add([ordered]@{
                name = $agentFile.BaseName
                sourcePath = $agentFile.FullName
            })
        }
    }

    # --- Provider capability flags ---
    $package.providerCapabilities = [ordered]@{
        agy = [ordered]@{
            skills = $true
            commandSkills = $true
            agents = $true
            instructions = $true
            mcp = $true
            hooks = $false
            runtimeScripts = $false
        }
        copilot = [ordered]@{
            skills = $true
            commandSkills = $true
            agents = $true
            instructions = $true
            mcp = $true
            hooks = $false
            runtimeScripts = $false
        }
        codex = [ordered]@{
            skills = $true
            commandSkills = $true
            agents = $false
            instructions = $true
            mcp = $true
            hooks = $false
            runtimeScripts = $false
        }
        claude = [ordered]@{
            skills = $true
            commandSkills = $true
            agents = $true
            instructions = $true
            mcp = $true
            hooks = $false
            runtimeScripts = $false
        }
    }

    # --- Skipped components for v1 ---
    $package.skippedComponents.Add('hooks')
    $package.skippedComponents.Add('runtimeScripts')

    return $package
}

function Test-ProviderPluginPackage {
    <#
    .SYNOPSIS
        Validates a provider-neutral plugin package against the shared substrate contract.
    #>
    param(
        [System.Collections.IDictionary]$Package
    )

    $errors = [System.Collections.Generic.List[string]]::new()

    # --- Reject provider-specific paths in the common model ---
    $providerSpecificPaths = @(
        '.codex-plugin'
        '.claude-plugin'
        'rules/'
        'mcp_config.json'
        'hooks.json'
        'gal-results/'
        'runtimeScripts'
        'scripts/'
    )

    $jsonText = $Package | ConvertTo-Json -Depth 20
    foreach ($path in $providerSpecificPaths) {
        if ($path -eq 'runtimeScripts') {
            # Count occurrences: should appear exactly once inside skippedComponents
            $matches = [regex]::Matches($jsonText, [regex]::Escape('"runtimeScripts"'))
            $inSkipped = $false
            foreach ($m in $matches) {
                $before = $jsonText.Substring(0, $m.Index)
                if ($before -match '"skippedComponents"') {
                    $inSkipped = $true
                }
                else {
                    $errors.Add("Provider-specific path '$path' leaked into provider-neutral package model")
                }
            }
            continue
        }
        if ($jsonText -match [regex]::Escape($path)) {
            $errors.Add("Provider-specific path '$path' leaked into provider-neutral package model")
        }
    }

    # --- Reject resolved local values ---
    $localValueIndicators = @('config.local.env', 'credentials', 'secret', 'password')
    foreach ($indicator in $localValueIndicators) {
        if ($jsonText -match [regex]::Escape($indicator)) {
            $errors.Add("Potential local-value indicator '$indicator' found in provider-neutral model")
        }
    }

    # --- Name collision check ---
    $skillNames = @($Package.skills | ForEach-Object { $_.name })
    $commandSkillNames = @($Package.commandSkills | ForEach-Object { $_.name })
    $allNames = $skillNames + $commandSkillNames
    $duplicates = $allNames | Group-Object | Where-Object { $_.Count -gt 1 }
    if ($duplicates) {
        foreach ($dup in $duplicates) {
            $errors.Add("Name collision detected: '$($dup.Name)' appears in both skills and commandSkills")
        }
    }

    # --- Source path existence ---
    foreach ($skill in $Package.skills) {
        if (-not (Test-Path $skill.sourcePath)) {
            $errors.Add("Skill source path does not exist: $($skill.sourcePath)")
        }
    }
    foreach ($cmdSkill in $Package.commandSkills) {
        if (-not (Test-Path $cmdSkill.sourcePath)) {
            $errors.Add("Command skill source path does not exist: $($cmdSkill.sourcePath)")
        }
    }
    foreach ($agent in $Package.agents) {
        if (-not (Test-Path $agent.sourcePath)) {
            $errors.Add("Agent source path does not exist: $($agent.sourcePath)")
        }
    }

    # --- Required fields ---
    if (-not $Package.metadata -or [string]::IsNullOrWhiteSpace($Package.metadata.name)) {
        $errors.Add('Package metadata.name is required')
    }

    if (-not $Package.packageSchema -or $Package.packageSchema.packageId -ne 'gal-core') {
        $errors.Add('Package packageSchema.packageId must be set to gal-core')
    }

    if (-not $Package.sourcePlugins -or @($Package.sourcePlugins | Where-Object { $_.pluginId -eq 'gal-core' }).Count -eq 0) {
        $errors.Add('Package sourcePlugins must include gal-core')
    }

    $disallowedCompanionPatterns = Get-GalCoreCopiedCompanionSkillPatterns
    foreach ($skill in $Package.skills) {
        foreach ($pattern in $disallowedCompanionPatterns) {
            if ($skill.name -like $pattern) {
                $errors.Add("Copied companion skill '$($skill.name)' leaked into gal-core canonical package")
            }
        }
    }

    # --- T-002: Unsupported component skip validation ---
    $requiredSkippedComponents = @('hooks', 'runtimeScripts')
    foreach ($component in $requiredSkippedComponents) {
        if ($Package.skippedComponents -notcontains $component) {
            $errors.Add("Required skipped component '$component' is not explicitly recorded in skippedComponents")
        }
    }

    # --- T-002: Verify no stubs for unsupported components ---
    # If hooks or runtimeScripts appear anywhere other than skippedComponents, it's a stub
    $jsonText = $Package | ConvertTo-Json -Depth 20
    $stubIndicators = @('"hooks"', '"runtimeScripts"')
    foreach ($indicator in $stubIndicators) {
        $matches = [regex]::Matches($jsonText, [regex]::Escape($indicator))
        foreach ($m in $matches) {
            $before = $jsonText.Substring(0, $m.Index)
            # If this occurrence is not inside skippedComponents, it's a stub
            if (-not ($before -match '"skippedComponents"')) {
                $errors.Add("Unsupported component stub detected: $indicator appears outside skippedComponents")
            }
        }
    }

    # --- T-002: Explicit gal-results/ check ---
    if ($jsonText -match [regex]::Escape('gal-results/')) {
        $errors.Add("gal-results/ path leaked into provider-neutral package model")
    }

    # --- T-002: Local-only artifact boundary validation ---
    # Ensure mcpSpec only carries boolean hasLocalOverrides, not resolved values
    if ($Package.mcpSpec -and $Package.mcpSpec -is [System.Collections.IDictionary]) {
        $mcpKeys = @($Package.mcpSpec.Keys)
        $allowedMcpKeys = @('canonicalSource', 'hasLocalOverrides')
        foreach ($key in $mcpKeys) {
            if ($allowedMcpKeys -notcontains $key) {
                $errors.Add("MCP spec contains disallowed key '$key'; only canonicalSource and hasLocalOverrides are permitted in the common model")
            }
        }
        # Ensure hasLocalOverrides is strictly boolean
        if ($Package.mcpSpec.Contains('hasLocalOverrides')) {
            $localOverridesValue = $Package.mcpSpec['hasLocalOverrides']
            if ($localOverridesValue -isnot [bool]) {
                $errors.Add("MCP spec hasLocalOverrides must be a boolean flag, not a resolved value")
            }
        }
    }

    # --- T-002: Provider capability flags validation ---
    if (-not $Package.providerCapabilities -or $Package.providerCapabilities -isnot [System.Collections.IDictionary]) {
        $errors.Add('Package providerCapabilities is required')
    }
    else {
        $requiredProviders = @('agy', 'copilot', 'codex', 'claude')
        foreach ($provider in $requiredProviders) {
            if (-not $Package.providerCapabilities.Contains($provider)) {
                $errors.Add("Provider capability flags missing for '$provider'")
            }
        }
    }

    return [pscustomobject]@{
        Valid = $errors.Count -eq 0
        Issues = @($errors)
    }
}

function Get-AgyPluginArtifactRoot {
    <#
    .SYNOPSIS
        Returns the generated artifact root for the AGY renderer.
    #>
    param([string]$RepoRoot)
    return Join-Path $RepoRoot 'dist/provider-plugins/agy/gal'
}

function Get-AgyPluginInstallTarget {
    <#
    .SYNOPSIS
        Returns the AGY plugin install target path.
    #>
    return Join-Path $env:USERPROFILE '.gemini/antigravity-cli/plugins/gal'
}
