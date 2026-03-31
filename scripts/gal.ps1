param(
    [Parameter(Position = 0)]
    [string]$Command,

    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Arguments
)

$ErrorActionPreference = "Stop"

function Show-Usage {
    Write-Host "Usage: gal <command> [args]"
    Write-Host ""
    Write-Host "Commands:"
    Write-Host "  init [targetPath] [projectName]     Initialize .dev/ and docs/plans/"
    Write-Host "  dispatch [subcommand|golem] [text]  Route to subcommand or golem via /gal skill"
    Write-Host ""
    Write-Host "Script-dispatched subcommands: init, research"
    Write-Host "Control-plane skills (use in chat): /gal status, /gal whats-next, /gal wrap-up"
}

function ConvertTo-Slug([string]$Value) {
    $slug = $Value.ToLowerInvariant()
    $slug = $slug -replace "[^a-z0-9]+", "-"
    $slug = $slug.Trim("-")

    if ([string]::IsNullOrWhiteSpace($slug)) {
        throw "Could not derive a slug from input: $Value"
    }

    return $slug
}

function Get-StatePath {
    return Join-Path (Get-Location).Path ".dev\state.md"
}

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptRoot

# --- Dispatch helpers ---

function Write-Dispatch([hashtable]$Fields) {
    Write-Host "--- GAL DISPATCH ---"
    foreach ($key in $Fields.Keys) {
        if ($null -ne $Fields[$key] -and $Fields[$key] -ne '') {
            Write-Host "${key}: $($Fields[$key])"
        }
    }
    Write-Host "--- END DISPATCH ---"
}

function Get-WFState {
    $statePath = Join-Path (Get-Location).Path ".dev\state.md"
    if (-not (Test-Path $statePath)) { return $null }
    $line = Get-Content $statePath | Where-Object { $_ -match '^Workflow:' } | Select-Object -First 1
    if ($line -match '^Workflow:\s*(\S+)') { return $Matches[1] }
    return $null
}

function Resolve-Golem([string]$Name) {
    $known = @('golem-planner','golem-architect','golem-analyst','golem-implementer',
               'golem-tester','golem-reviewer','golem-verifier','golem-debugger',
               'golem-scribe','golem-librarian','golem-designer','golem-researcher')
    # accept with or without 'golem-' prefix
    $full = if ($Name -like 'golem-*') { $Name } else { "golem-$Name" }
    if ($known -contains $full) { return $full }
    return $null
}

$dispatchByState = @{
    'PLAN'         = 'golem-planner'
    'DISCUSS'      = 'golem-architect'
    'IMPLEMENT'    = 'golem-implementer'
    'TEST'         = 'golem-tester'
    'REVIEW'       = 'golem-reviewer'
    'CROSS_REVIEW' = 'golem-reviewer'
    'VERIFY'       = 'golem-verifier'
}

$workflowBindings = @{
    'golem-planner'     = @('PLAN')
    'golem-implementer' = @('IMPLEMENT')
    'golem-tester'      = @('TEST')
    'golem-reviewer'    = @('REVIEW', 'CROSS_REVIEW')
    'golem-verifier'    = @('VERIFY')
}

$domainGolems  = @('golem-architect','golem-analyst','golem-librarian','golem-designer','golem-researcher')
$utilityGolems = @('golem-debugger','golem-scribe')

switch ($Command) {
    "init" {
        & (Join-Path $scriptRoot "Init-Repo.ps1") @Arguments
        break
    }
    "dispatch" {
        $intent  = if ($Arguments.Count -gt 0) { $Arguments[0] } else { '' }
        $subText = if ($Arguments.Count -gt 1) { $Arguments[1..($Arguments.Count-1)] -join ' ' } else { '' }

        $subcommands = @('init','research')
        if ($subcommands -contains $intent) {
            $action = "Execute the $intent workflow step."
            $onComplete = 'Report result to user.'

            switch ($intent) {
                'init' {
                    $action = 'Initialize .dev/ for the target repo, then surface the manual next step.'
                    $onComplete = 'Tell the user to review .dev/project.md, then invoke /gal status for next steps.'
                }
                'research' {
                    $action = 'Activate the /gal research skill for structured investigation.'
                    $onComplete = 'Synthesize findings and surface RESEARCH_COMPLETE to the user.'
                }
            }

            # Delegate to subcommand handler and wrap output
            Write-Host "--- GAL DISPATCH ---"
            Write-Host "COMMAND: $intent"
            Write-Host "ACTION: $action"
            Write-Host "ON_COMPLETE: $onComplete"
            Write-Host "--- END DISPATCH ---"
            break
        }

        $resolved = if ($intent) { Resolve-Golem $intent } else { $null }
        if ($resolved) {
            $state = Get-WFState
            if ($utilityGolems -contains $resolved) {
                $mode = 'utility'
            } elseif ($domainGolems -contains $resolved) {
                $mode = 'consult'
            } else {
                $boundStates = $workflowBindings[$resolved]
                $mode = if ($boundStates -and $boundStates -contains $state) { 'bound' } else { 'consult' }
            }
            $action = if ($subText) { $subText } else { "Invoke $resolved — awaiting user instruction." }
            Write-Dispatch @{
                ROLE         = $resolved
                MODE         = $mode
                ACTION       = $action
                ON_COMPLETE  = 'Report result to user.'
            }
            break
        }

        if ($intent -and $subcommands -notcontains $intent) {
            Write-Dispatch @{
                COMMAND = 'error'
                ACTION = "Unknown argument: '$intent'. Use a subcommand (init/research) or a golem name."
            }
            break
        }

        # Auto-detect from state
        $state = Get-WFState
        if (-not $state) {
            Write-Dispatch @{
                COMMAND = 'suggest'
                ACTION  = 'No .dev/state.md found. Run /gal init to initialize this repository.'
                ON_COMPLETE = 'Run /gal init'
            }
            break
        }
        if ($state -eq 'IDLE') {
            Write-Dispatch @{
                COMMAND = 'suggest'
                ACTION  = "Workflow is IDLE. Use /office-hours or /autoplan to create a plan, or /gal status for details."
                ON_COMPLETE = 'Run /gal status or /office-hours'
            }
            break
        }
            $golem = $dispatchByState[$state]
        if ($golem) {
            $agentPath = Join-Path $repoRoot ("agent\{0}.agent.md" -f $golem)
            Write-Dispatch @{
                ROLE        = $golem
                MODE        = 'bound'
                READ        = $agentPath
                ACTION      = "Workflow state is $state. Activate $golem in bound mode."
                ON_COMPLETE = 'Update .dev/state.md and suggest next step.'
            }
        } else {
            Write-Dispatch @{
                COMMAND = 'suggest'
                ACTION  = "Workflow state '$state' has no default golem. Use '/gal <golem-name>' to invoke directly."
            }
        }
        break
    }
    default {
        if ($Command) {
            Write-Host "ERROR: Unsupported top-level command '$Command'. Use 'gal init' or 'gal dispatch'." -ForegroundColor Red
            Write-Host "Control-plane actions like /gal status, /gal whats-next, and /gal wrap-up run in chat."
            Write-Host ""
            Show-Usage
            exit 1
        }
        Show-Usage
    }
}