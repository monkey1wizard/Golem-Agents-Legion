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
    Write-Host "  init [targetPath] [projectName]   Initialize .dev/ and docs/plans/"
    Write-Host "  plan [-Type <type>] <name>        Create a draft plan file from template"
    Write-Host "  status                            Show current workflow status from .dev/state.md"
    Write-Host "  next                              Show the next step from .dev/state.md"
    Write-Host "  pause                             Commit .dev/ context for worktree handoff"
    Write-Host "  sync                              Run Sync-DevContext if available"
    Write-Host "  dispatch [subcommand|golem] [text] Auto-detect state or invoke named golem"
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
    "plan" {
        $planType = "feat"
        $nameArgs = @()

        for ($i = 0; $i -lt $Arguments.Count; $i++) {
            if ($Arguments[$i] -eq "-Type" -and ($i + 1) -lt $Arguments.Count) {
                $planType = $Arguments[$i + 1]
                $i++
            } else {
                $nameArgs += $Arguments[$i]
            }
        }

        if ($nameArgs.Count -eq 0) {
            throw "Usage: gal plan [-Type <type>] <name>"
        }

        $planName = ($nameArgs -join " ").Trim()
        $slug = ConvertTo-Slug $planName
        $plansDir = Join-Path (Get-Location).Path "docs\plans"
        $planTemplatePath = Join-Path $repoRoot "templates\plan.md"
        $planTargetPath = Join-Path $plansDir ("{0}-{1}.prompt.md" -f $planType, $slug)

        if (-not (Test-Path $planTemplatePath)) {
            throw "Missing template: $planTemplatePath"
        }

        New-Item -ItemType Directory -Force -Path $plansDir | Out-Null

        if (Test-Path $planTargetPath) {
            throw "Plan already exists: $planTargetPath"
        }

        $content = Get-Content -Path $planTemplatePath -Raw
        $content = $content -replace "\[Feature Name\]", $planName
        Set-Content -Path $planTargetPath -Value $content

        Write-Host "Created plan scaffold: $planTargetPath"
        Write-Host "Next: classify T0/T1/T2 and fill Review Pack before implementation."
        break
    }
    "status" {
        $statePath = Get-StatePath
        if (-not (Test-Path $statePath)) {
            throw "Missing .dev/state.md in current directory. Run gal init first."
        }
        $stateContent = Get-Content -Path $statePath
        $interestingLines = $stateContent | Where-Object {
            $_ -match "^Workflow:" -or $_ -match "^Plan:" -or $_ -match "^Step:" -or $_ -match "^Last activity:" -or $_ -match "^Last session:" -or $_ -match "^Next step:"
        }

        Write-Host ("State file: {0}" -f $statePath)
        $interestingLines | ForEach-Object { Write-Host $_ }
        break
    }
    "next" {
        $statePath = Get-StatePath
        if (-not (Test-Path $statePath)) {
            throw "Missing .dev/state.md in current directory. Run gal init first."
        }
        $nextLine = Get-Content -Path $statePath | Where-Object { $_ -match "^Next step:" } | Select-Object -First 1

        if (-not $nextLine) {
            throw "No 'Next step:' entry found in $statePath"
        }

        Write-Host $nextLine
        break
    }
    "pause" {
        $statePath = Get-StatePath
        if (-not (Test-Path $statePath)) {
            throw "Missing .dev/state.md in current directory. Run gal init first."
        }
        $devDir = Join-Path (Get-Location).Path ".dev"
        $plansDir = Join-Path (Get-Location).Path "docs\plans"

        Write-Host "=== gal pause: Context Handoff ==="
        Write-Host ""
        Write-Host "Before running this command, ask your AI session to:"
        Write-Host "  1. Write key context to plan's ## Status > ### Handoff Notes"
        Write-Host "  2. Update .dev/state.md Session Continuity"
        Write-Host ""

        $hasChanges = $false
        $gitStatus = git status --porcelain -- $devDir $plansDir 2>$null
        if ($gitStatus) {
            $hasChanges = $true
        }

        if (-not $hasChanges) {
            Write-Host "No uncommitted changes in .dev/ or docs/plans/. Nothing to commit."
            break
        }

        git add $devDir $plansDir
        git commit -m "chore: gal pause — context handoff"
        Write-Host ""
        Write-Host "Committed .dev/ and docs/plans/ changes. Safe to switch worktree."
        break
    }
    "sync" {
        $syncScriptPath = Join-Path $scriptRoot "Sync-DevContext.ps1"
        if (-not (Test-Path $syncScriptPath)) {
            throw "Sync-DevContext.ps1 is not implemented yet. Command surface reserved; adapter generation still pending."
        }

        & $syncScriptPath @Arguments
        break
    }
    "dispatch" {
        $intent  = if ($Arguments.Count -gt 0) { $Arguments[0] } else { '' }
        $subText = if ($Arguments.Count -gt 1) { $Arguments[1..($Arguments.Count-1)] -join ' ' } else { '' }

        $subcommands = @('init','plan','status','next','pause','sync')
        if ($subcommands -contains $intent) {
            $action = "Execute the $intent workflow step."
            $onComplete = 'Report result to user.'

            switch ($intent) {
                'init' {
                    $action = 'Initialize .dev/ for the target repo, then surface the manual next step.'
                    $onComplete = 'Tell the user to review .dev/project.md, curate ## Active Skills, then run /gal sync.'
                }
                'sync' {
                    $action = 'Generate .github/copilot-instructions.md and GEMINI.md from .dev/project.md.'
                    $onComplete = 'Report which adapter files were generated and whether Active Skills validation passed.'
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
                ACTION = "Unknown argument: '$intent'. Use a subcommand (init/plan/status/next/pause/sync) or a golem name."
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
                ACTION  = "Workflow is IDLE. Run '/gal plan' to create a plan or '/gal status' for details."
                ON_COMPLETE = 'Run /gal plan'
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
        Show-Usage
        if ($Command) {
            exit 1
        }
    }
}