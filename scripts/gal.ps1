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
    Write-Host "  state-dump                          Dump .dev/state.md + active plan files for LLM skills"
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

function Unwrap-MarkdownCode([string]$Value) {
    if ($null -eq $Value) { return $null }
    $trimmed = $Value.Trim()
    if ($trimmed.Length -ge 2 -and $trimmed.StartsWith('`') -and $trimmed.EndsWith('`')) {
        return $trimmed.Substring(1, $trimmed.Length - 2)
    }
    return $trimmed
}

function Resolve-PlanPath([string]$Value) {
    $candidate = Unwrap-MarkdownCode $Value
    if ([string]::IsNullOrWhiteSpace($candidate)) { return $null }

    $candidate = $candidate -replace '/', '\'
    $absolutePath = if ([System.IO.Path]::IsPathRooted($candidate)) {
        $candidate
    } else {
        Join-Path (Get-Location).Path $candidate
    }

    if ($absolutePath -match '\.prompt\.md$') {
        return $absolutePath
    }

    if ($absolutePath -match '\.md$') {
        $promptPath = $absolutePath -replace '\.md$', '.prompt.md'
        if (Test-Path $promptPath) {
            return $promptPath
        }
    }

    if (Test-Path $absolutePath) {
        return $absolutePath
    }

    return $absolutePath
}

function Get-ActivePlanPath {
    $statePath = Get-StatePath
    if (-not (Test-Path $statePath)) { return $null }

    $inActivePlans = $false
    $headerSeen = $false

    foreach ($line in Get-Content $statePath) {
        if ($line -match '^##\s+Active Plans\b') {
            $inActivePlans = $true
            continue
        }

        if ($inActivePlans -and $line -match '^##\s+') {
            break
        }

        if (-not $inActivePlans) { continue }
        if ($line -notmatch '^\|') { continue }
        if ($line -match '^\|\s*---') { continue }

        $cells = $line.Trim().Trim('|').Split('|') | ForEach-Object { $_.Trim() }
        if ($cells.Count -lt 2) { continue }

        if (-not $headerSeen) {
            if ($cells -contains 'Plan' -and $cells -contains 'Workflow State') {
                $headerSeen = $true
            }
            continue
        }

        foreach ($cell in $cells) {
            $resolved = Resolve-PlanPath $cell
            if ($resolved -and $resolved -match '\.(prompt\.md|md)$') {
                return $resolved
            }
        }
    }

    return $null
}

function Get-PlanStatusField([string]$PlanPath, [string]$FieldName) {
    if ([string]::IsNullOrWhiteSpace($PlanPath) -or -not (Test-Path $PlanPath)) { return $null }

    $inStatus = $false
    $pattern = '^{0}:\s*(.+)$' -f [regex]::Escape($FieldName)

    foreach ($line in Get-Content $PlanPath) {
        if ($line -match '^##\s+Status\b') {
            $inStatus = $true
            continue
        }

        if ($inStatus -and $line -match '^##\s+') {
            break
        }

        if ($inStatus -and $line -match $pattern) {
            return $Matches[1].Trim()
        }
    }

    return $null
}

function Normalize-WorkflowState([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value)) { return $null }

    $normalized = $Value.Trim().ToUpperInvariant()
    switch ($normalized) {
        'ENG-REVIEWED' { return 'IMPLEMENT' }
        'APPROVED'     { return 'IMPLEMENT' }
        default        { return $normalized }
    }
}

function Get-StateContext {
    $statePath = Get-StatePath
    if (-not (Test-Path $statePath)) {
        return [pscustomobject]@{
            Kind = 'uninitialized'
            WorkflowState = $null
            WorkflowStateRaw = $null
            ActivePlanPath = $null
            Error = $null
        }
    }

    $activePlanPath = Get-ActivePlanPath
    if (-not $activePlanPath) {
        return [pscustomobject]@{
            Kind = 'idle'
            WorkflowState = 'IDLE'
            WorkflowStateRaw = 'IDLE'
            ActivePlanPath = $null
            Error = $null
        }
    }

    if (-not (Test-Path $activePlanPath)) {
        return [pscustomobject]@{
            Kind = 'state-error'
            WorkflowState = $null
            WorkflowStateRaw = $null
            ActivePlanPath = $activePlanPath
            Error = "Active plan file not found: $activePlanPath"
        }
    }

    $workflowRaw = Get-PlanStatusField -PlanPath $activePlanPath -FieldName 'Workflow'
    $workflowState = Normalize-WorkflowState $workflowRaw
    if (-not $workflowState) {
        return [pscustomobject]@{
            Kind = 'state-error'
            WorkflowState = $null
            WorkflowStateRaw = $workflowRaw
            ActivePlanPath = $activePlanPath
            Error = "Could not resolve workflow state from $activePlanPath `## Status`."
        }
    }

    return [pscustomobject]@{
        Kind = if ($workflowState -eq 'IDLE') { 'idle' } else { 'active' }
        WorkflowState = $workflowState
        WorkflowStateRaw = $workflowRaw
        ActivePlanPath = $activePlanPath
        Error = $null
    }
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
    "state-dump" {
        # Deterministic file dump for LLM skills — outputs all relevant state files
        # so the LLM never needs to decide whether to call a file-reading tool.
        Write-Host "--- GAL STATE DUMP ---"

        $context = Get-StateContext
        Write-Host "KIND: $($context.Kind)"
        Write-Host "WORKFLOW: $($context.WorkflowState)"
        Write-Host "WORKFLOW_RAW: $($context.WorkflowStateRaw)"
        Write-Host "ACTIVE_PLAN: $($context.ActivePlanPath)"
        if ($context.Error) {
            Write-Host "ERROR: $($context.Error)"
        }

        # Dump .dev/state.md
        $statePath = Get-StatePath
        if (Test-Path $statePath) {
            Write-Host ""
            Write-Host "--- FILE: .dev/state.md ---"
            Get-Content $statePath | ForEach-Object { Write-Host $_ }
            Write-Host "--- END FILE ---"
        }

        # Dump active plan file
        if ($context.ActivePlanPath -and (Test-Path $context.ActivePlanPath)) {
            $relPath = $context.ActivePlanPath
            $cwd = (Get-Location).Path
            if ($relPath.StartsWith($cwd)) {
                $relPath = $relPath.Substring($cwd.Length).TrimStart('\', '/')
            }
            Write-Host ""
            Write-Host "--- FILE: $relPath ---"
            Get-Content $context.ActivePlanPath | ForEach-Object { Write-Host $_ }
            Write-Host "--- END FILE ---"
        }

        # Dump .dev/project.md if it exists
        $projectPath = Join-Path (Get-Location).Path ".dev\project.md"
        if (Test-Path $projectPath) {
            Write-Host ""
            Write-Host "--- FILE: .dev/project.md ---"
            Get-Content $projectPath | ForEach-Object { Write-Host $_ }
            Write-Host "--- END FILE ---"
        }

        Write-Host ""
        Write-Host "--- END STATE DUMP ---"
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
            $context = Get-StateContext
            $state = $context.WorkflowState
            if ($utilityGolems -contains $resolved) {
                $mode = 'utility'
            } elseif ($domainGolems -contains $resolved) {
                $mode = 'consult'
            } else {
                $boundStates = $workflowBindings[$resolved]
                $mode = if ($context.Kind -eq 'active' -and $boundStates -and $boundStates -contains $state) { 'bound' } else { 'consult' }
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
        $context = Get-StateContext
        $state = $context.WorkflowState
        if ($context.Kind -eq 'uninitialized') {
            Write-Dispatch @{
                COMMAND = 'suggest'
                ACTION  = 'No .dev/state.md found. Run /gal init to initialize this repository.'
                ON_COMPLETE = 'Run /gal init'
            }
            break
        }
        if ($context.Kind -eq 'idle') {
            Write-Dispatch @{
                COMMAND = 'suggest'
                ACTION  = "Repo is initialized but no active workflow is recorded. Use /office-hours or /autoplan to create a plan, or /gal status for details."
                ON_COMPLETE = 'Run /gal status or /office-hours'
            }
            break
        }
        if ($context.Kind -eq 'state-error') {
            Write-Dispatch @{
                COMMAND = 'suggest'
                ACTION  = "Repo is initialized, but GAL could not resolve workflow state from the active plan. Inspect .dev/state.md Active Plans and $($context.ActivePlanPath) `## Status`."
                ON_COMPLETE = 'Fix repo state, then run /gal status'
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
