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

$commonHelpersScript = Join-Path $PSScriptRoot 'Common.ps1'
if (Test-Path $commonHelpersScript) {
    . $commonHelpersScript
}

function Get-GalCoreCanonicalPackageSchema {
    <#
    .SYNOPSIS
        Returns the Claude-compatible canonical package schema for gal-core.
    #>

    return [ordered]@{
        schemaId = 'gal-plugin-root-v2'
        schemaVersion = 1
        packageId = 'gal-core'
        packageKind = 'canonical-plugin'
        canonicalProvider = 'claude'
        compatibleProviders = @('claude', 'copilot', 'codex', 'agy')
        canonicalLayout = '.gal/plugins/<plugin-id>'
        componentRoots = [ordered]@{
            claudeManifest = '.claude-plugin/plugin.json'
            codexManifest = '.codex-plugin/plugin.json'
            skills = 'skills'
            commands = 'commands'
            agents = 'agents'
            hooks = 'hooks'
            mcp = 'provider-managed'
            lsp = 'provider-managed'
            app = 'provider-managed'
            assets = 'assets'
        }
        nativeInstallProviders = @('claude', 'copilot', 'codex')
        managedShortcutProviders = @('agy')
    }
}

function Get-GalCanonicalPluginRootRecord {
    param(
        [string]$PluginId = 'gal'
    )

    return [ordered]@{
        pluginId = $PluginId
        relativeRoot = '.gal/plugins/{0}' -f $PluginId
        absoluteRoot = Get-GalPluginRoot -PluginId $PluginId
        dataRoot = Get-GalPluginDataRoot -PluginId $PluginId
    }
}

function Get-GalCoreCopiedCompanionSkillPatterns {
    <#
    .SYNOPSIS
        Returns copied skill prefixes that belong to companion plugins rather than gal-core.
    #>

    return @('dart-*', 'flutter-*')
}

function Get-LocalOverrideSkillEntries {
    param([string]$GalXmachineConfigFile)

    if (-not (Test-Path $GalXmachineConfigFile)) {
        return @()
    }

    $binding = Read-JsonOrderedMap $GalXmachineConfigFile
    if ($null -eq $binding -or -not $binding.Contains('localPluginPaths')) {
        return @()
    }

    $entries = [System.Collections.Generic.List[object]]::new()
    foreach ($pluginPath in @($binding['localPluginPaths'])) {
        if ([string]::IsNullOrWhiteSpace([string]$pluginPath)) {
            continue
        }

        $skillsRoot = Join-Path ([string]$pluginPath) 'skills'
        if (-not (Test-Path $skillsRoot)) {
            continue
        }

        foreach ($skillDir in Get-ChildItem $skillsRoot -Directory | Sort-Object Name) {
            $skillFile = Join-Path $skillDir.FullName 'SKILL.md'
            if (-not (Test-Path $skillFile)) {
                continue
            }

            $entries.Add([ordered]@{
                name = $skillDir.Name
                sourcePath = $skillFile
            })
        }
    }

    return @($entries)
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
            canonicalPluginRoot = Get-GalCanonicalPluginRootRecord
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
    $knownSkillNames = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
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
                [void]$knownSkillNames.Add($skillDir.Name)
                $package.skills.Add([ordered]@{
                    name = $skillDir.Name
                    sourcePath = $skillFile
                })
            }
        }
    }

    foreach ($skill in (Get-LocalOverrideSkillEntries -GalXmachineConfigFile (Join-Path $env:USERPROFILE '.gal\config\xmachine.json'))) {
        if ($knownSkillNames.Contains([string]$skill['name'])) {
            continue
        }

        [void]$knownSkillNames.Add([string]$skill['name'])
        $package.skills.Add($skill)
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
    $mcpLocalFile = Join-Path $env:USERPROFILE '.gal\config\mcp.local.json'
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
            $pathTokens = [regex]::Matches($jsonText, [regex]::Escape('"runtimeScripts"'))
            foreach ($token in $pathTokens) {
                $before = $jsonText.Substring(0, $token.Index)
                if ($before -match '"skippedComponents"') {
                    continue
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

    # --- T-002: Verify no unsupported component stubs are emitted as package payload ---
    $unexpectedRootComponents = @('hooks', 'runtimeScripts')
    foreach ($component in $unexpectedRootComponents) {
        if ($Package.Contains($component) -and $null -ne $Package[$component]) {
            $errors.Add("Unsupported component stub detected: '$component' is emitted as package payload")
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

function Get-AgyPluginPackageOutputRoot {
    <#
    .SYNOPSIS
        Returns the AGY package output root under ~/.gal/dist.
    #>
    param([string]$RepoRoot)
    return Join-Path $env:USERPROFILE '.gal\dist\provider-plugins\agy\gal'
}

function Get-AgyPluginArtifactRoot {
    <#
    .SYNOPSIS
        Backward-compatible alias for the AGY package output root.
    #>
    param([string]$RepoRoot)
    return Get-AgyPluginPackageOutputRoot -RepoRoot $RepoRoot
}

function Get-AgyPluginInstallTarget {
    <#
    .SYNOPSIS
        Returns the AGY plugin install target path.
    #>
    return Join-Path $env:USERPROFILE '.gemini/antigravity-cli/plugins/gal'
}

function Get-ClaudePluginPackageOutputRoot {
    <#
    .SYNOPSIS
        Returns the Claude package output root under ~/.gal/dist.
    #>
    param([string]$RepoRoot)
    return Join-Path $env:USERPROFILE '.gal\dist\provider-plugins\claude\gal'
}

function Get-ClaudePluginArtifactRoot {
    <#
    .SYNOPSIS
        Backward-compatible alias for the Claude package output root.
    #>
    param([string]$RepoRoot)
    return Get-ClaudePluginPackageOutputRoot -RepoRoot $RepoRoot
}

function Get-ClaudePluginComponentRelativePaths {
    <#
    .SYNOPSIS
        Returns the Claude plugin component layout relative to the plugin root.
    #>

    return [ordered]@{
        manifest = '.claude-plugin/plugin.json'
        skills = 'skills'
        commands = 'commands'
        agents = 'agents'
        mcp = '.mcp.json'
    }
}

function Get-ClaudePluginManifestPath {
    <#
    .SYNOPSIS
        Returns the manifest path for a rendered Claude plugin artifact.
    #>
    param([string]$PluginRoot)

    $componentPaths = Get-ClaudePluginComponentRelativePaths
    return Join-Path $PluginRoot $componentPaths.manifest
}

function Get-ClaudePluginInstallContract {
    <#
    .SYNOPSIS
        Returns the documented Claude plugin install and validation contract.
    #>

    return [ordered]@{
        developmentLoadCommand = 'claude --plugin-dir <plugin-root>'
        validationCommand = 'claude plugin validate <plugin-root> --strict'
        lifecycleCommands = @(
            'claude plugin install <plugin> --scope <scope>'
            'claude plugin update <plugin> --scope <scope>'
            'claude plugin uninstall <plugin> --scope <scope>'
        )
        settingsScopes = [ordered]@{
            user = '~/.claude/settings.json'
            project = '.claude/settings.json'
            local = '.claude/settings.local.json'
            managed = 'managed settings'
        }
        cacheRoot = Join-Path $env:USERPROFILE '.claude/plugins/cache'
        dataRoot = Join-Path $env:USERPROFILE '.claude/plugins/data'
        notes = @(
            'Only .claude-plugin/plugin.json belongs inside .claude-plugin; all other plugin components stay at plugin root.',
            'Plugin artifact rendering and user-scope Claude CLI lifecycle operations are distinct concerns.',
            'Plugin data is persistent across updates and is deleted when the last install scope is removed unless --keep-data is used.'
        )
    }
}
