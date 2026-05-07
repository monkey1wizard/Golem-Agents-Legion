#Requires -Version 5.1
param(
    [switch]$Uninstall,
    [switch]$Replace,
    [switch]$DryRun,
    [switch]$Reconfigure,
    [string[]]$SelectedRuntimes,
    [string]$PrimaryRuntime
)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'common\Common.ps1')

function Get-McpVariableMap([System.Collections.IDictionary]$LocalEnvValues) {
    $values = [ordered]@{}
    foreach ($key in $LocalEnvValues.Keys) {
        $values[$key] = $LocalEnvValues[$key]
    }

    if (-not $values.Contains('MCP_MEMORY_FILE_PATH')) {
        $values['MCP_MEMORY_FILE_PATH'] = Join-Path $env:USERPROFILE 'mcp-memory.json'
    }

    if (-not $values.Contains('MCP_FILESYSTEM_PATHS')) {
        $paths = @((Split-Path $script:SetupContext.RepoRoot -Parent))
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
        return ,([object[]]$items.ToArray())
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

function Test-JsonLikeEqual([object]$Left, [object]$Right) {
    return ((ConvertTo-OrderedMap $Left | ConvertTo-Json -Depth 20) -eq (ConvertTo-OrderedMap $Right | ConvertTo-Json -Depth 20))
}

function ConvertTo-GeminiMcpConfig([System.Collections.IDictionary]$Config) {
    $converted = [ordered]@{}

    foreach ($key in $Config.Keys) {
        if ($key -eq 'type') { continue }
        if ($key -eq 'url' -and $Config.Contains('type') -and [string]$Config['type'] -eq 'http') {
            $converted['httpUrl'] = [string]$Config['url']
            continue
        }

        $converted[$key] = $Config[$key]
    }

    return $converted
}

function ConvertTo-AntigravityMcpConfig([System.Collections.IDictionary]$Config) {
    $converted = [ordered]@{}

    foreach ($key in $Config.Keys) {
        if ($key -eq 'type') { continue }
        if ($key -eq 'url') {
            $converted['serverUrl'] = [string]$Config['url']
            continue
        }

        $converted[$key] = $Config[$key]
    }

    return $converted
}

function ConvertTo-CodexMcpConfig([System.Collections.IDictionary]$Config) {
    $converted = [ordered]@{}
    foreach ($key in $Config.Keys) {
        if ($key -eq 'type') { continue }
        $converted[$key] = $Config[$key]
    }
    return $converted
}

function Get-CopilotCliBridgeProfile([string]$ServerName) {
    switch ($ServerName) {
        'chromedevtools/chrome-devtools-mcp' { return [ordered]@{ Enabled = $true; Key = 'chrome-devtools' } }
        'github-mcp-server' { return [ordered]@{ Enabled = $false; Key = $null } }
        'memory' { return [ordered]@{ Enabled = $true; Key = 'memory' } }
        'microsoftdocs/mcp' { return [ordered]@{ Enabled = $true; Key = 'microsoftdocs' } }
        'microsoft/markitdown' { return [ordered]@{ Enabled = $true; Key = 'markitdown' } }
        'upstash/context7' { return [ordered]@{ Enabled = $true; Key = 'context7' } }
        'blender' { return [ordered]@{ Enabled = $true; Key = 'blender' } }
        'freecad' { return [ordered]@{ Enabled = $true; Key = 'freecad' } }
        default {
            $normalized = $ServerName -replace '^[^A-Za-z0-9]+', '' -replace '[^A-Za-z0-9_-]+', '-'
            if ([string]::IsNullOrWhiteSpace($normalized)) {
                $normalized = 'server'
            }
            return [ordered]@{ Enabled = $true; Key = $normalized.ToLowerInvariant() }
        }
    }
}

function Get-CopilotCliTransport([System.Collections.IDictionary]$Config) {
    if ($Config.Contains('type')) {
        switch ([string]$Config['type']) {
            'http' { return 'http' }
            'sse' { return 'sse' }
            'stdio' { return 'local' }
            'local' { return 'local' }
            default { return [string]$Config['type'] }
        }
    }

    if ($Config.Contains('url')) { return 'http' }
    if ($Config.Contains('command')) { return 'local' }

    return 'local'
}

function ConvertTo-CopilotCliMcpConfig([System.Collections.IDictionary]$Config) {
    $transport = Get-CopilotCliTransport $Config
    $converted = [ordered]@{
        type = $transport
        tools = @('*')
    }

    if ($transport -eq 'http' -or $transport -eq 'sse') {
        if ($Config.Contains('url')) {
            $converted['url'] = [string]$Config['url']
        }
        if ($Config.Contains('headers') -and $Config['headers'] -is [System.Collections.IDictionary]) {
            $converted['headers'] = $Config['headers']
        }
        return $converted
    }

    if ($Config.Contains('command')) {
        $converted['command'] = [string]$Config['command']
    }

    $normalizedArgs = [System.Collections.Generic.List[object]]::new()
    if ($Config.Contains('args') -and $null -ne $Config['args']) {
        if ($Config['args'] -is [System.Collections.IEnumerable] -and -not ($Config['args'] -is [string])) {
            foreach ($arg in $Config['args']) {
                $normalizedArgs.Add([string]$arg)
            }
        }
        else {
            $normalizedArgs.Add([string]$Config['args'])
        }
    }
    $converted['args'] = [object[]]$normalizedArgs.ToArray()

    if ($Config.Contains('env') -and $Config['env'] -is [System.Collections.IDictionary]) {
        $converted['env'] = $Config['env']
    }

    return $converted
}

function ConvertTo-OpenCodeMcpConfig([System.Collections.IDictionary]$Config) {
    $transport = if ($Config.Contains('type')) { [string]$Config['type'] } elseif ($Config.Contains('url')) { 'http' } else { 'stdio' }
    if ($transport -eq 'http' -or $transport -eq 'sse') {
        $converted = [ordered]@{
            type = 'remote'
            url = [string]$Config['url']
            enabled = $true
        }
        if ($Config.Contains('headers') -and $Config['headers'] -is [System.Collections.IDictionary]) {
            $converted['headers'] = $Config['headers']
        }
        if ($Config.Contains('timeout')) {
            $converted['timeout'] = $Config['timeout']
        }
        return $converted
    }

    $command = [System.Collections.Generic.List[object]]::new()
    if ($Config.Contains('command')) {
        $command.Add([string]$Config['command'])
    }
    if ($Config.Contains('args') -and $Config['args'] -is [System.Collections.IEnumerable] -and -not ($Config['args'] -is [string])) {
        foreach ($arg in $Config['args']) {
            $command.Add([string]$arg)
        }
    }
    elseif ($Config.Contains('args') -and $null -ne $Config['args']) {
        $command.Add([string]$Config['args'])
    }

    $converted = [ordered]@{
        type = 'local'
        command = [object[]]$command.ToArray()
        enabled = $true
    }
    if ($Config.Contains('env') -and $Config['env'] -is [System.Collections.IDictionary]) {
        $converted['environment'] = $Config['env']
    }
    if ($Config.Contains('timeout')) {
        $converted['timeout'] = $Config['timeout']
    }
    return $converted
}

function ConvertTo-TomlString([string]$Value) {
    $escaped = $Value.Replace('\\', '\\\\').Replace('"', '\\"')
    return '"' + $escaped + '"'
}

function Format-TomlKeySegment([string]$Key) {
    if ($Key -match '^[A-Za-z0-9_-]+$') {
        return $Key
    }

    return (ConvertTo-TomlString $Key)
}

function ConvertTo-TomlValue([object]$Value) {
    if ($null -eq $Value) {
        return '""'
    }

    if ($Value -is [string]) {
        return ConvertTo-TomlString $Value
    }

    if ($Value -is [bool]) {
        return $Value.ToString().ToLowerInvariant()
    }

    if ($Value -is [ValueType]) {
        return [System.Convert]::ToString($Value, [System.Globalization.CultureInfo]::InvariantCulture)
    }

    if ($Value -is [System.Collections.IEnumerable] -and -not ($Value -is [string])) {
        return '[' + ((@($Value) | ForEach-Object { ConvertTo-TomlValue $_ }) -join ', ') + ']'
    }

    return ConvertTo-TomlString ([string]$Value)
}

function ConvertTo-TomlTableSections([string[]]$PathSegments, [System.Collections.IDictionary]$Data) {
    $header = '[' + (($PathSegments | ForEach-Object { Format-TomlKeySegment $_ }) -join '.') + ']'
    $lines = @($header)
    $nestedSections = @()

    foreach ($key in $Data.Keys) {
        $value = $Data[$key]
        if ($value -is [System.Collections.IDictionary]) {
            $nestedSections += ConvertTo-TomlTableSections -PathSegments ($PathSegments + @($key)) -Data $value
            continue
        }

        $lines += ('{0} = {1}' -f $key, (ConvertTo-TomlValue $value))
    }

    return @(($lines -join "`r`n")) + @($nestedSections)
}

function ConvertTo-CodexMcpSection([string]$ServerName, [System.Collections.IDictionary]$Config) {
    return (ConvertTo-TomlTableSections -PathSegments @('mcp_servers', $ServerName) -Data $Config) -join "`r`n`r`n"
}

function Get-CodexTableServerName([string]$Line) {
    if ($Line -notmatch '^\[(?<path>[^\]]+)\]\s*$') { return $null }

    $path = $matches['path']
    if (-not $path.StartsWith('mcp_servers.')) { return $null }

    $remainder = $path.Substring('mcp_servers.'.Length)
    if ([string]::IsNullOrWhiteSpace($remainder)) { return $null }

    if ($remainder.StartsWith('"')) {
        $builder = New-Object System.Text.StringBuilder
        for ($index = 1; $index -lt $remainder.Length; $index++) {
            $char = $remainder[$index]
            if ($char -eq '"' -and $remainder[$index - 1] -ne '\\') {
                return $builder.ToString().Replace('\\"', '"').Replace('\\\\', '\\')
            }
            [void]$builder.Append($char)
        }

        return $null
    }

    if ($remainder -match '^(?<name>[^\.]+)') {
        return $matches['name']
    }

    return $remainder
}

function Remove-CodexManagedServersFromToml([string]$RawContent, [string[]]$ServerNames) {
    if ([string]::IsNullOrWhiteSpace($RawContent)) {
        return ''
    }

    $managedNames = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($serverName in $ServerNames) {
        [void]$managedNames.Add($serverName)
    }

    $result = New-Object System.Collections.Generic.List[string]
    $skipCurrentSection = $false

    foreach ($line in ($RawContent -split "`r?`n")) {
        $serverName = Get-CodexTableServerName $line
        if ($null -ne $serverName) {
            $skipCurrentSection = $managedNames.Contains($serverName)
        }
        elseif ($line -match '^\[[^\]]+\]\s*$') {
            $skipCurrentSection = $false
        }

        if (-not $skipCurrentSection) {
            [void]$result.Add($line)
        }
    }

    return ($result -join "`r`n").TrimEnd("`r", "`n")
}

function Get-LegacyManagedMcpAliases([string]$RuntimeName, [string]$ServerName) {
    switch ($RuntimeName) {
        'gemini' {
            switch ($ServerName) {
                'upstash/context7' { return @('context7') }
                'microsoftdocs/mcp' { return @('Microsoft Learn MCP Server') }
                'github/github-mcp-server' { return @('github') }
                'chromedevtools/chrome-devtools-mcp' { return @('chrome-devtools') }
            }
        }
        'codex' {
            switch ($ServerName) {
                'upstash/context7' { return @('context7') }
                'microsoftdocs/mcp' { return @('microsoftdocs') }
                'imageFetch' { return @('imagefetch') }
                'github/github-mcp-server' { return @('github') }
                'chromedevtools/chrome-devtools-mcp' { return @('chrome-devtools') }
            }
        }
    }

    return @()
}

function Get-ResolvedManagedMcpManifest {
    $context = $script:SetupContext
    if (-not (Test-Path $context.McpSourceFile)) {
        Write-Host "  [WARN] MCP source file not found: $($context.McpSourceFile)" -ForegroundColor Yellow
        return $null
    }

    if (-not (Test-Path $context.McpLocalFile) -and -not $script:SetupOptions.Uninstall -and -not $script:SetupOptions.DryRun) {
        Write-JsonOrderedMap $context.McpLocalFile ([ordered]@{ servers = [ordered]@{} })
        Write-Host "  [OK] Created local MCP override file: $($context.McpLocalFile)"
    }

    $manifest = Read-JsonOrderedMap $context.McpSourceFile
    if ($null -eq $manifest) { return $null }

    if (Test-Path $context.McpLocalFile) {
        $localManifest = Read-JsonOrderedMap $context.McpLocalFile
        if ($null -eq $localManifest) { return $null }
        $manifest = Merge-OrderedMap $manifest $localManifest
    }

    if (-not $manifest.Contains('servers') -or $manifest['servers'] -isnot [System.Collections.IDictionary]) {
        Write-Host '  [WARN] MCP manifest does not contain a valid servers object.' -ForegroundColor Yellow
        return $null
    }

    $mcpVariables = Get-McpVariableMap (Read-KeyValueEnvFile (Join-Path $context.RepoRoot 'config.local.env'))
    $resolvedServers = [ordered]@{}
    foreach ($serverName in $manifest['servers'].Keys) {
        $resolvedServers[$serverName] = Resolve-McpConfig $manifest['servers'][$serverName] $mcpVariables
    }

    return [ordered]@{
        servers = $resolvedServers
    }
}

function Update-VscodeMcpConfig([System.Collections.IDictionary]$ManagedManifest) {
    $context = $script:SetupContext
    $vscodeMcp = Read-JsonOrderedMap $context.VscodeMcpFile
    if ($null -eq $vscodeMcp) { return }

    if (-not $vscodeMcp.Contains('servers') -or $vscodeMcp['servers'] -isnot [System.Collections.IDictionary]) {
        $vscodeMcp['servers'] = [ordered]@{}
    }

    $changed = $false
    foreach ($serverName in $ManagedManifest['servers'].Keys) {
        $resolvedConfig = $ManagedManifest['servers'][$serverName]
        if (-not $vscodeMcp['servers'].Contains($serverName) -or -not (Test-JsonLikeEqual $vscodeMcp['servers'][$serverName] $resolvedConfig)) {
            $vscodeMcp['servers'][$serverName] = $resolvedConfig
            $changed = $true
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would set VS Code MCP server: $serverName"
            }
            else {
                Write-Host "  [SET] VS Code MCP server: $serverName"
            }
        }
    }

    if ($changed -and -not $script:SetupOptions.DryRun) {
        Write-JsonOrderedMap $context.VscodeMcpFile $vscodeMcp
        Write-Host "  [OK] $($context.VscodeMcpFile)"
    }
}

function Update-CopilotCliMcpConfig([System.Collections.IDictionary]$ManagedManifest) {
    $context = $script:SetupContext
    $copilotCliMcp = Read-JsonOrderedMap $context.CopilotCliMcpFile
    if ($null -eq $copilotCliMcp) { return }

    if (-not $copilotCliMcp.Contains('mcpServers') -or $copilotCliMcp['mcpServers'] -isnot [System.Collections.IDictionary]) {
        $copilotCliMcp['mcpServers'] = [ordered]@{}
    }

    $managedBridgeConfigs = foreach ($serverName in $ManagedManifest['servers'].Keys) {
        Get-CopilotCliBridgeProfile -ServerName $serverName
    }

    $managedKeys = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($bridgeConfig in @($managedBridgeConfigs)) {
        if ($null -ne $bridgeConfig['Key'] -and -not [string]::IsNullOrWhiteSpace([string]$bridgeConfig['Key'])) {
            [void]$managedKeys.Add([string]$bridgeConfig['Key'])
        }
    }
    foreach ($legacyName in @('chromedevtools/chrome-devtools-mcp', 'github-mcp-server', 'microsoftdocs/mcp', 'microsoft/markitdown', 'upstash/context7')) {
        [void]$managedKeys.Add($legacyName)
    }

    $changed = $false
    foreach ($existingServerName in @($copilotCliMcp['mcpServers'].Keys)) {
        if ($managedKeys.Contains([string]$existingServerName)) {
            $copilotCliMcp['mcpServers'].Remove($existingServerName)
            $changed = $true
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would remove Copilot CLI MCP server: $existingServerName"
            }
            else {
                Write-Host "  [CLEANUP] Copilot CLI MCP server removed: $existingServerName"
            }
        }
    }

    foreach ($serverName in $ManagedManifest['servers'].Keys) {
        $bridgeConfig = Get-CopilotCliBridgeProfile -ServerName $serverName
        if (-not $bridgeConfig['Enabled']) {
            continue
        }

        $targetServerName = [string]$bridgeConfig['Key']
        $converted = ConvertTo-CopilotCliMcpConfig $ManagedManifest['servers'][$serverName]
        if (-not $copilotCliMcp['mcpServers'].Contains($targetServerName) -or -not (Test-JsonLikeEqual $copilotCliMcp['mcpServers'][$targetServerName] $converted)) {
            $copilotCliMcp['mcpServers'][$targetServerName] = $converted
            $changed = $true
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would set Copilot CLI MCP server: $targetServerName"
            }
            else {
                Write-Host "  [SET] Copilot CLI MCP server: $targetServerName"
            }
        }
    }

    if ($changed -and -not $script:SetupOptions.DryRun) {
        Write-JsonOrderedMap $context.CopilotCliMcpFile $copilotCliMcp
        Write-Host "  [OK] $($context.CopilotCliMcpFile)"
    }
}

function Update-GeminiMcpConfig([System.Collections.IDictionary]$ManagedManifest) {
    $context = $script:SetupContext
    $geminiSettings = Read-JsonOrderedMap $context.GeminiSettingsFile
    if ($null -eq $geminiSettings) { return }

    if (-not $geminiSettings.Contains('mcpServers') -or $geminiSettings['mcpServers'] -isnot [System.Collections.IDictionary]) {
        $geminiSettings['mcpServers'] = [ordered]@{}
    }

    $changed = $false
    foreach ($serverName in $ManagedManifest['servers'].Keys) {
        foreach ($legacyAlias in (Get-LegacyManagedMcpAliases -RuntimeName 'gemini' -ServerName $serverName)) {
            if ($geminiSettings['mcpServers'].Contains($legacyAlias)) {
                $geminiSettings['mcpServers'].Remove($legacyAlias)
                $changed = $true
                if ($script:SetupOptions.DryRun) {
                    Write-Host "  [DRY RUN] Would remove Gemini MCP alias: $legacyAlias"
                }
                else {
                    Write-Host "  [CLEANUP] Gemini MCP alias removed: $legacyAlias"
                }
            }
        }

        $converted = ConvertTo-GeminiMcpConfig $ManagedManifest['servers'][$serverName]
        if (-not $geminiSettings['mcpServers'].Contains($serverName) -or -not (Test-JsonLikeEqual $geminiSettings['mcpServers'][$serverName] $converted)) {
            $geminiSettings['mcpServers'][$serverName] = $converted
            $changed = $true
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would set Gemini MCP server: $serverName"
            }
            else {
                Write-Host "  [SET] Gemini MCP server: $serverName"
            }
        }
    }

    if ($changed -and -not $script:SetupOptions.DryRun) {
        Write-JsonOrderedMap $context.GeminiSettingsFile $geminiSettings
        Write-Host "  [OK] $($context.GeminiSettingsFile)"
    }
}

function Update-AntigravityMcpConfig([System.Collections.IDictionary]$ManagedManifest) {
    $context = $script:SetupContext
    $antigravityConfig = Read-JsonOrderedMap $context.AntigravityMcpFile
    if ($null -eq $antigravityConfig) {
        $antigravityConfig = [ordered]@{}
    }

    if (-not $antigravityConfig.Contains('mcpServers') -or $antigravityConfig['mcpServers'] -isnot [System.Collections.IDictionary]) {
        $antigravityConfig['mcpServers'] = [ordered]@{}
    }

    $changed = $false
    foreach ($serverName in $ManagedManifest['servers'].Keys) {
        $converted = ConvertTo-AntigravityMcpConfig $ManagedManifest['servers'][$serverName]
        if (-not $antigravityConfig['mcpServers'].Contains($serverName) -or -not (Test-JsonLikeEqual $antigravityConfig['mcpServers'][$serverName] $converted)) {
            $antigravityConfig['mcpServers'][$serverName] = $converted
            $changed = $true
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would set Antigravity MCP server: $serverName"
            }
            else {
                Write-Host "  [SET] Antigravity MCP server: $serverName"
            }
        }
    }

    if ($changed -and -not $script:SetupOptions.DryRun) {
        Write-JsonOrderedMap $context.AntigravityMcpFile $antigravityConfig
        Write-Host "  [OK] $($context.AntigravityMcpFile)"
    }
}

function Update-CodexMcpConfig([System.Collections.IDictionary]$ManagedManifest) {
    $context = $script:SetupContext
    $codexConfigDir = Split-Path $context.CodexConfigFile -Parent
    if (-not (Test-Path $codexConfigDir) -and -not $script:SetupOptions.DryRun) {
        New-Item -ItemType Directory -Path $codexConfigDir -Force | Out-Null
    }

    $codexRaw = if (Test-Path $context.CodexConfigFile) { Get-Content $context.CodexConfigFile -Raw -Encoding UTF8 } else { '' }
    $managedNames = New-Object System.Collections.Generic.List[string]
    foreach ($serverName in $ManagedManifest['servers'].Keys) {
        [void]$managedNames.Add($serverName)
        foreach ($legacyAlias in (Get-LegacyManagedMcpAliases -RuntimeName 'codex' -ServerName $serverName)) {
            [void]$managedNames.Add($legacyAlias)
        }
    }

    $remainingRaw = Remove-CodexManagedServersFromToml -RawContent $codexRaw -ServerNames @($managedNames)
    $sections = foreach ($serverName in $ManagedManifest['servers'].Keys) {
        ConvertTo-CodexMcpSection -ServerName $serverName -Config (ConvertTo-CodexMcpConfig $ManagedManifest['servers'][$serverName])
    }

    $newCodexRaw = $remainingRaw
    if (-not [string]::IsNullOrWhiteSpace($newCodexRaw) -and -not $newCodexRaw.EndsWith("`r`n`r`n")) {
        $newCodexRaw = $newCodexRaw.TrimEnd("`r", "`n") + "`r`n`r`n"
    }
    $newCodexRaw += ($sections -join "`r`n`r`n") + "`r`n"

    if ($script:SetupOptions.DryRun) {
        foreach ($serverName in $ManagedManifest['servers'].Keys) {
            Write-Host "  [DRY RUN] Would set Codex MCP server: $serverName"
        }
        return
    }

    [System.IO.File]::WriteAllText($context.CodexConfigFile, $newCodexRaw, $context.Utf8NoBom)
    foreach ($serverName in $ManagedManifest['servers'].Keys) {
        Write-Host "  [SET] Codex MCP server: $serverName"
    }
    Write-Host "  [OK] $($context.CodexConfigFile)"
}

function Update-OpenCodeMcpConfig([System.Collections.IDictionary]$ManagedManifest) {
    $context = $script:SetupContext
    $openCodeConfig = Read-JsonOrderedMap $context.OpenCodeConfigFile
    if ($null -eq $openCodeConfig) {
        $openCodeConfig = [ordered]@{}
    }

    if (-not $openCodeConfig.Contains('mcp') -or $openCodeConfig['mcp'] -isnot [System.Collections.IDictionary]) {
        $openCodeConfig['mcp'] = [ordered]@{}
    }

    $changed = $false
    foreach ($serverName in $ManagedManifest['servers'].Keys) {
        $converted = ConvertTo-OpenCodeMcpConfig $ManagedManifest['servers'][$serverName]
        if (-not $openCodeConfig['mcp'].Contains($serverName) -or -not (Test-JsonLikeEqual $openCodeConfig['mcp'][$serverName] $converted)) {
            $openCodeConfig['mcp'][$serverName] = $converted
            $changed = $true
            if ($script:SetupOptions.DryRun) {
                Write-Host "  [DRY RUN] Would set OpenCode MCP server: $serverName"
            }
            else {
                Write-Host "  [SET] OpenCode MCP server: $serverName"
            }
        }
    }

    if ($changed -and -not $script:SetupOptions.DryRun) {
        Write-JsonOrderedMap $context.OpenCodeConfigFile $openCodeConfig
        Write-Host "  [OK] $($context.OpenCodeConfigFile)"
    }
}

function Invoke-ClaudeMcpCommand([string[]]$Arguments) {
    & claude @Arguments
    return $LASTEXITCODE
}

function Update-ClaudeMcpConfig([System.Collections.IDictionary]$ManagedManifest) {
    if (-not (Test-CommandAvailable 'claude')) {
        Write-Host '  [WARN] Claude CLI not found; skipping Claude MCP installation.' -ForegroundColor Yellow
        return
    }

    foreach ($serverName in $ManagedManifest['servers'].Keys) {
        $resolvedConfig = $ManagedManifest['servers'][$serverName]
        if ($script:SetupOptions.DryRun) {
            Write-Host "  [DRY RUN] Would upsert Claude MCP server: $serverName"
            continue
        }

        Invoke-ClaudeMcpCommand -Arguments @('mcp', 'remove', $serverName) | Out-Null

        $exitCode = 0
        if ($resolvedConfig.Contains('type') -and [string]$resolvedConfig['type'] -eq 'http') {
            $arguments = @('mcp', 'add', '--scope', 'user', '--transport', 'http')
            if ($resolvedConfig.Contains('headers') -and $resolvedConfig['headers'] -is [System.Collections.IDictionary]) {
                foreach ($headerName in $resolvedConfig['headers'].Keys) {
                    $arguments += @('-H', ('{0}: {1}' -f $headerName, $resolvedConfig['headers'][$headerName]))
                }
            }
            if ($resolvedConfig.Contains('tools')) {
                Write-Host "  [WARN] Claude CLI does not expose a tools filter flag; installing $serverName without tools scoping." -ForegroundColor Yellow
            }

            $arguments += @($serverName, [string]$resolvedConfig['url'])
            $exitCode = Invoke-ClaudeMcpCommand -Arguments $arguments
        }
        else {
            $arguments = @('mcp', 'add', '--scope', 'user', '--transport', 'stdio')
            if ($resolvedConfig.Contains('env') -and $resolvedConfig['env'] -is [System.Collections.IDictionary]) {
                foreach ($envName in $resolvedConfig['env'].Keys) {
                    $arguments += @('-e', ('{0}={1}' -f $envName, $resolvedConfig['env'][$envName]))
                }
            }

            $commandAndArgs = @([string]$resolvedConfig['command'])
            if ($resolvedConfig.Contains('args')) {
                $commandAndArgs += @($resolvedConfig['args'] | ForEach-Object { [string]$_ })
            }
            $arguments += @($serverName, '--') + $commandAndArgs
            $exitCode = Invoke-ClaudeMcpCommand -Arguments $arguments
        }

        if ($exitCode -eq 0) {
            Write-Host "  [SET] Claude MCP server: $serverName"
        }
        else {
            Write-Host "  [WARN] Failed to configure Claude MCP server: $serverName" -ForegroundColor Yellow
        }
    }
}

function Invoke-UpdateMcp {
    $context = $script:SetupContext

    Write-Host ''
    Write-Host '=== MCP config bridge ==='

    if ($script:SetupOptions.Uninstall) {
        Write-Host '  [SKIP] MCP config files are preserved during uninstall.'
        return
    }

    if ($script:SetupOptions.DryRun) {
        if ($context.InstallCopilot) {
            Write-Host "  [DRY RUN] Would merge MCP servers into: $($context.VscodeMcpFile)"
            Write-Host "  [DRY RUN] Would merge MCP servers into: $($context.CopilotCliMcpFile)"
        }
        if ($context.InstallGemini) { Write-Host "  [DRY RUN] Would merge MCP servers into: $($context.GeminiSettingsFile)" }
        if ($context.InstallAntigravity) { Write-Host "  [DRY RUN] Would merge MCP servers into: $($context.AntigravityMcpFile)" }
        if ($context.InstallCodex) { Write-Host "  [DRY RUN] Would merge MCP servers into: $($context.CodexConfigFile)" }
        if ($context.InstallOpenCode) { Write-Host "  [DRY RUN] Would merge MCP servers into: $($context.OpenCodeConfigFile)" }
        if ($context.InstallClaude) { Write-Host '  [DRY RUN] Would merge MCP servers through Claude CLI user scope' }
    }

    $manifest = Get-ResolvedManagedMcpManifest
    if ($null -eq $manifest) { return }

    if ($context.InstallCopilot) {
        Update-VscodeMcpConfig -ManagedManifest $manifest
        Update-CopilotCliMcpConfig -ManagedManifest $manifest
    }
    if ($context.InstallGemini) { Update-GeminiMcpConfig -ManagedManifest $manifest }
    if ($context.InstallAntigravity) { Update-AntigravityMcpConfig -ManagedManifest $manifest }
    if ($context.InstallCodex) { Update-CodexMcpConfig -ManagedManifest $manifest }
    if ($context.InstallOpenCode) { Update-OpenCodeMcpConfig -ManagedManifest $manifest }
    if ($context.InstallClaude) { Update-ClaudeMcpConfig -ManagedManifest $manifest }
}

Initialize-SetupSession -EntryScriptPath $MyInvocation.MyCommand.Path -Uninstall:$Uninstall -Replace:$Replace -DryRun:$DryRun -Reconfigure:$Reconfigure -SelectedRuntimes $SelectedRuntimes -PrimaryRuntime $PrimaryRuntime | Out-Null
Invoke-UpdateMcp