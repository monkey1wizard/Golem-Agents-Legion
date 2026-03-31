<#
.SYNOPSIS
    Removes all symlinks created by Setup-Machine.ps1.

.DESCRIPTION
    Wrapper around Setup-Machine.ps1 -Uninstall.
    Removes:
      - ~/.copilot/agents/*.agent.md symlinks
      - ~/.copilot/skills/*/ symlinks
      - ~/.gemini/skills/*/ symlinks
    - ~/.agents/skills/*/ symlinks
            - ~/.copilot/skills/gal*/ symlinks (GAL dispatcher + aliases)
            - ~/.gemini/skills/gal*/ symlinks (GAL dispatcher + aliases)
        - ~/.agents/skills/gal*/ symlinks (GAL dispatcher + aliases)
      - ~/.copilot/gal/ symlink (GAL_ROOT)
      - ~/.gemini/gal/ symlink (GAL_ROOT)
            - commands/gal*/SKILL.md (baked from template)
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
