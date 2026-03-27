<#
.SYNOPSIS
    Removes all symlinks created by Setup-Machine.ps1.

.DESCRIPTION
    Wrapper around Setup-Machine.ps1 -Uninstall.
    Removes:
      - ~/.copilot/agents/*.agent.md symlinks
      - ~/.copilot/skills/*/ symlinks
      - ~/.gemini/skills/*/ symlinks
      - ~/.gemini/gal-context.md

.PARAMETER DryRun
    Show what would be removed without making changes.

.EXAMPLE
    .\scripts\Uninstall-Machine.ps1
    .\scripts\Uninstall-Machine.ps1 -DryRun
#>
param(
    [switch]$DryRun
)

& "$PSScriptRoot\Setup-Machine.ps1" -Uninstall @PSBoundParameters
