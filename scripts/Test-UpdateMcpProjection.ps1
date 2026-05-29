#Requires -Version 5.1

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Assert-True {
    param(
        [bool]$Condition,
        [string]$Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

function Assert-Equal {
    param(
        $Actual,
        $Expected,
        [string]$Message
    )

    if ($Actual -ne $Expected) {
        throw "$Message`nExpected: $Expected`nActual: $Actual"
    }
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$scriptUnderTest = Join-Path $repoRoot 'scripts\Update-Mcp.ps1'
$testHome = Join-Path $env:TEMP ("gal-mcp-projection-test-{0}" -f [System.Guid]::NewGuid().ToString('N'))
$testAppData = Join-Path $testHome 'AppData\Roaming'
$originalUserProfile = $env:USERPROFILE
$originalHome = $env:HOME
$originalAppData = $env:APPDATA

New-Item -ItemType Directory -Path $testHome | Out-Null
New-Item -ItemType Directory -Path $testAppData | Out-Null

try {
    $env:USERPROFILE = $testHome
    $env:HOME = $testHome
    $env:APPDATA = $testAppData

    $galRoot = Join-Path $testHome '.gal'
    $galConfigRoot = Join-Path $galRoot 'config'
    $generatedRoot = Join-Path $galRoot 'generated'
    $copilotRoot = Join-Path $testHome '.copilot'
    $vscodeRoot = Join-Path $testAppData 'Code\User'

    New-Item -ItemType Directory -Path $galConfigRoot -Force | Out-Null
    New-Item -ItemType Directory -Path $generatedRoot -Force | Out-Null
    New-Item -ItemType Directory -Path $copilotRoot -Force | Out-Null
    New-Item -ItemType Directory -Path $vscodeRoot -Force | Out-Null

    $machineConfigPath = Join-Path $galConfigRoot 'config.json'
    $machineConfig = [ordered]@{
        schemaVersion = 1
        installMode = 'install'
        obsidianVault = 'C:\Vault'
        context7ApiKey = 'ctx-secret'
        providerSelections = [ordered]@{
            copilot = [ordered]@{ enabled = $true; lane = 'primary' }
        }
    }
    $machineConfig | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $machineConfigPath -Encoding utf8

    $xmachineBindingPath = Join-Path $galConfigRoot 'xmachine.json'
    $xmachineBinding = [ordered]@{
        schemaVersion = 1
        defaultXmachineNode = 'node-name'
        xmachineNodeAliases = [ordered]@{
            'node-name' = [ordered]@{
                target = 'user@host'
                repoPath = '/repo'
                runtimeRepoPath = '/runtime'
            }
        }
        machineProfiles = [ordered]@{}
        localPluginPaths = @('/plugins/local')
        providerPathOverrides = [ordered]@{
            copilot = '/providers/copilot'
        }
        additionalBindings = [ordered]@{
            smoke = 'enabled'
        }
    }
    $xmachineBinding | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $xmachineBindingPath -Encoding utf8

    $vscodeMcpPath = Join-Path $vscodeRoot 'mcp.json'
    $vscodeMcp = [ordered]@{
        servers = [ordered]@{
            memory = [ordered]@{
                command = 'npx'
                args = @('-y', '@modelcontextprotocol/server-memory')
                env = [ordered]@{
                    MEMORY_FILE_PATH = (Join-Path $testHome 'state\mcp-memory.json')
                }
            }
            'user-owned' = [ordered]@{
                command = 'custom-user-mcp'
                args = @('--keep')
            }
            github = [ordered]@{
                type = 'http'
                url = 'https://example.com/user-owned-github'
            }
        }
    }
    $vscodeMcp | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $vscodeMcpPath -Encoding utf8

    $copilotMcpPath = Join-Path $copilotRoot 'mcp-config.json'
    $copilotMcp = [ordered]@{
        mcpServers = [ordered]@{
            memory = [ordered]@{
                type = 'local'
                tools = @('*')
                command = 'npx'
                args = @('-y', '@modelcontextprotocol/server-memory')
                env = [ordered]@{
                    MEMORY_FILE_PATH = (Join-Path $testHome 'state\mcp-memory.json')
                }
            }
            'user-owned' = [ordered]@{
                command = 'custom-user-mcp'
                args = @('--keep')
            }
            context7 = [ordered]@{
                type = 'http'
                url = 'https://example.com/user-owned-context7'
                tools = @('*')
            }
        }
    }
    $copilotMcp | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $copilotMcpPath -Encoding utf8

    & $scriptUnderTest -SelectedRuntimes @('copilot') -PrimaryRuntime 'copilot' | Out-Null

    $generatedMcpPath = Join-Path $galRoot 'generated\mcp\managed.json'
    $generatedXmachinePath = Join-Path $galRoot 'generated\xmachine\managed.json'

    Assert-True (Test-Path $generatedMcpPath) 'Update-Mcp should write the managed MCP projection.'
    Assert-True (Test-Path $generatedXmachinePath) 'Update-Mcp should write the managed xmachine projection.'

    $generatedMcp = Get-Content -LiteralPath $generatedMcpPath -Raw -Encoding UTF8 | ConvertFrom-Json
    Assert-True ($generatedMcp.PSObject.Properties.Name -notcontains 'mcpMemoryFilePath') 'Managed MCP projection should not emit deprecated mcpMemoryFilePath state.'
    Assert-True ($generatedMcp.mcpServers.PSObject.Properties.Name -notcontains 'memory') 'Managed MCP projection should not include the removed memory MCP server.'
    Assert-Equal $generatedMcp.mcpServers.'upstash/context7'.headers.CONTEXT7_API_KEY 'ctx-secret' 'Context7 auth header should be materialized only in generated MCP state.'
    Assert-True ($generatedMcp.PSObject.Properties.Name -notcontains 'mcpFilesystemPaths') 'mcpFilesystemPaths should be omitted when filesystem MCP is absent.'
    Assert-True ([bool]$generatedMcp._metadata.secretBearing) 'Managed MCP projection metadata should mark secret-bearing state.'

    $generatedXmachine = Get-Content -LiteralPath $generatedXmachinePath -Raw -Encoding UTF8 | ConvertFrom-Json
    Assert-Equal $generatedXmachine.defaultXmachineNode 'node-name' 'Managed xmachine projection should preserve the default node.'
    Assert-Equal $generatedXmachine.nodes.'node-name'.target 'user@host' 'Managed xmachine projection should materialize xmachine node aliases.'

    $updatedVscodeMcp = Get-Content -LiteralPath $vscodeMcpPath -Raw -Encoding UTF8 | ConvertFrom-Json
    Assert-Equal $updatedVscodeMcp.servers.'user-owned'.command 'custom-user-mcp' 'VS Code MCP update should preserve unrelated user-owned entries.'
    Assert-True ($updatedVscodeMcp.servers.PSObject.Properties.Name -notcontains 'memory') 'VS Code MCP update should not install the removed memory MCP server.'
    Assert-Equal $updatedVscodeMcp.servers.github.url 'https://example.com/user-owned-github' 'VS Code MCP update should preserve conflicting user-owned keys it does not already own.'

    $updatedCopilotMcp = Get-Content -LiteralPath $copilotMcpPath -Raw -Encoding UTF8 | ConvertFrom-Json
    Assert-Equal $updatedCopilotMcp.mcpServers.'user-owned'.command 'custom-user-mcp' 'Copilot CLI MCP update should preserve unrelated user-owned entries.'
    Assert-True ($updatedCopilotMcp.mcpServers.PSObject.Properties.Name -contains 'chrome-devtools') 'Copilot CLI MCP update should install enabled managed bridge entries.'
    Assert-True ($updatedCopilotMcp.mcpServers.PSObject.Properties.Name -notcontains 'github') 'Copilot CLI MCP update should not install disabled GitHub bridge entries.'
    Assert-True ($updatedCopilotMcp.mcpServers.PSObject.Properties.Name -notcontains 'github-mcp-server') 'Copilot CLI MCP update should remove the disabled legacy GitHub bridge entry.'
    Assert-True ($updatedCopilotMcp.mcpServers.PSObject.Properties.Name -notcontains 'memory') 'Copilot CLI MCP update should not install the removed memory MCP server.'
    Assert-Equal $updatedCopilotMcp.mcpServers.context7.url 'https://example.com/user-owned-context7' 'Copilot CLI MCP update should preserve conflicting user-owned keys it does not already own.'

    Write-Host 'PASS: MCP/xmachine generated projections and user-owned MCP preservation verified.'
}
finally {
    $env:USERPROFILE = $originalUserProfile
    $env:HOME = $originalHome
    $env:APPDATA = $originalAppData

    if (Test-Path $testHome) {
        Remove-Item -LiteralPath $testHome -Recurse -Force
    }
}
