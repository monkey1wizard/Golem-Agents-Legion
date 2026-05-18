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
    Write-Host "  xmachine <node> to do <task-ref>    Run one active-plan task on a readied work node"
    Write-Host ""
    Write-Host "Script-dispatched subcommands: init, research, deep-research"
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

function Get-RepoContextRoot {
    $current = (Get-Location).Path

    while ($true) {
        $statePath = Join-Path $current ".dev\state.md"
        if (Test-Path $statePath) {
            return $current
        }

        $parent = Split-Path $current -Parent
        if ([string]::IsNullOrWhiteSpace($parent) -or $parent -eq $current) {
            return (Get-Location).Path
        }

        $current = $parent
    }
}

function Get-StatePath {
    return Join-Path (Get-RepoContextRoot) ".dev\state.md"
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
    $repoContextRoot = Get-RepoContextRoot
    $absolutePath = if ([System.IO.Path]::IsPathRooted($candidate)) {
        $candidate
    } else {
        Join-Path $repoContextRoot $candidate
    }

    if ($absolutePath -match '\.prompt\.md$') {
        return $absolutePath
    }

    if ($absolutePath -match '\.md$') {
        $sourcePlanRoot = Join-Path $repoContextRoot 'docs\plans'
        if ($absolutePath.StartsWith($sourcePlanRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
            $relativeSourcePlanPath = $absolutePath.Substring($sourcePlanRoot.Length).TrimStart('\\')
            $relativePromptPath = $relativeSourcePlanPath -replace '\.md$', '.prompt.md'
            $devPromptPath = Join-Path (Join-Path $repoContextRoot '.dev\plans') $relativePromptPath
            if (Test-Path $devPromptPath) {
                return $devPromptPath
            }
        }

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
            if ($cells -contains 'Plan' -and (($cells -contains 'Workflow State') -or ($cells -contains 'Plan Phase'))) {
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

    return [pscustomobject]@{
        Kind = 'active'
        WorkflowState = if ([string]::IsNullOrWhiteSpace($workflowRaw)) { $null } else { $workflowRaw.Trim().ToUpperInvariant() }
        WorkflowStateRaw = $workflowRaw
        ActivePlanPath = $activePlanPath
        Error = $null
    }
}

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptRoot
$xmachineDocPath = Join-Path $repoRoot "docs\collaborative-tools\xmachine.md"

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

function Read-XmachineNodeAliases {
    $configPath = Join-Path $repoRoot "xmachine.config.json"
    if (-not (Test-Path $configPath)) {
        return @()
    }

    try {
        $config = Get-Content $configPath -Raw | ConvertFrom-Json
    }
    catch {
        throw "Invalid JSON in '$configPath'. $($_.Exception.Message)"
    }

    if ($null -eq $config -or -not $config.PSObject.Properties.Name.Contains('nodes')) {
        return @()
    }

    $nodes = $config.nodes
    if ($null -eq $nodes) {
        return @()
    }

    return @($nodes.PSObject.Properties.Name | ForEach-Object { [string]$_ })
}

function Get-XmachineDispatchContext {
    param([string[]]$Tokens)

    $aliases = Read-XmachineNodeAliases
    $requested = $false
    $workNode = $null

    foreach ($token in $Tokens) {
        if ([string]::IsNullOrWhiteSpace($token)) {
            continue
        }

        if ($token -in @('xmachine', '--xmachine')) {
            $requested = $true
            continue
        }

        if ($aliases -contains $token -and -not $workNode) {
            $workNode = $token
        }
    }

    return [pscustomobject]@{
        Requested      = $requested
        WorkNode       = $workNode
        AvailableNodes = $aliases
    }
}

function Get-ExplicitPlanArgument {
    param([string[]]$Tokens)

    foreach ($token in $Tokens) {
        if ([string]::IsNullOrWhiteSpace($token)) {
            continue
        }

        $candidate = $token.Trim()
        if ($candidate.StartsWith('#file:', [System.StringComparison]::OrdinalIgnoreCase)) {
            $candidate = $candidate.Substring(6)
        }

        if ($candidate -match '\.(prompt\.md|md)$') {
            return $candidate
        }
    }

    return $null
}

function Get-XmachineTaskShorthandContext {
    param([string[]]$Tokens)

    $availableNodes = Read-XmachineNodeAliases
    if (-not $Tokens -or $Tokens.Count -lt 4) {
        return [pscustomobject]@{
            Error = "Usage: gal xmachine <node> to do <task-ref> [#file:plan]"
            WorkNode = $null
            TaskRef = $null
            ExplicitPlan = $null
        }
    }

    $workNode = $Tokens[0]
    if ($availableNodes -notcontains $workNode) {
        $availableNodesText = if ($availableNodes.Count -gt 0) { $availableNodes -join ', ' } else { '<none configured>' }
        return [pscustomobject]@{
            Error = "Unknown xmachine work node '$workNode'. Available aliases: $availableNodesText"
            WorkNode = $workNode
            TaskRef = $null
            ExplicitPlan = $null
        }
    }

    if ($Tokens[1] -ne 'to' -or $Tokens[2] -ne 'do') {
        return [pscustomobject]@{
            Error = "Usage: gal xmachine <node> to do <task-ref> [#file:plan]"
            WorkNode = $workNode
            TaskRef = $null
            ExplicitPlan = $null
        }
    }

    $taskRef = $Tokens[3]
    if ([string]::IsNullOrWhiteSpace($taskRef)) {
        return [pscustomobject]@{
            Error = "Missing task reference. Usage: gal xmachine <node> to do <task-ref> [#file:plan]"
            WorkNode = $workNode
            TaskRef = $null
            ExplicitPlan = $null
        }
    }

    return [pscustomobject]@{
        Error = $null
        WorkNode = $workNode
        TaskRef = $taskRef.Trim()
        ExplicitPlan = Get-ExplicitPlanArgument -Tokens $Tokens
    }
}

function Resolve-Golem([string]$Name) {
    $known = @('golem-architect','golem-analyst','golem-implementer',
               'golem-tester','golem-reviewer','golem-verifier','golem-debugger',
               'golem-notewriter','golem-designer','golem-researcher')
    # accept with or without 'golem-' prefix
    $full = if ($Name -like 'golem-*') { $Name } else { "golem-$Name" }
    if ($known -contains $full) { return $full }
    return $null
}

$domainGolems  = @('golem-architect','golem-analyst','golem-designer','golem-researcher')
$utilityGolems = @('golem-debugger','golem-notewriter')

switch ($Command) {
    "init" {
        & (Join-Path $scriptRoot "Init-Repo.ps1") @Arguments
        break
    }
    "xmachine" {
        $shorthand = Get-XmachineTaskShorthandContext -Tokens $Arguments
        if ($shorthand.Error) {
            Write-Dispatch @{
                COMMAND = 'error'
                ACTION = $shorthand.Error
            }
            break
        }

        $dispatchFields = [ordered]@{
            COMMAND = 'pipeline'
            ACTION = "Follow the /gal-pipeline procedure to execute only task '$($shorthand.TaskRef)' on xmachine work node '$($shorthand.WorkNode)'. Resolve the active plan, scope execution to this single task, and keep control-plane convergence local."
            ON_COMPLETE = 'Report the single-task verdict and whether local convergence is complete.'
            READ = $xmachineDocPath
            EXECUTION = 'xmachine'
            WORK_NODE = $shorthand.WorkNode
            TASK_REF = $shorthand.TaskRef
            FROM = $shorthand.TaskRef
            STOP_AT = $shorthand.TaskRef
        }

        if ($shorthand.ExplicitPlan) {
            $dispatchFields['PLAN'] = $shorthand.ExplicitPlan
        }

        Write-Dispatch $dispatchFields
        break
    }
    "dispatch" {
        $intent  = if ($Arguments.Count -gt 0) { $Arguments[0] } else { '' }
        $subText = if ($Arguments.Count -gt 1) { $Arguments[1..($Arguments.Count-1)] -join ' ' } else { '' }
        $dispatchTokens = if ($Arguments.Count -gt 1) { $Arguments[1..($Arguments.Count-1)] } else { @() }
        $xmachineContext = Get-XmachineDispatchContext -Tokens $dispatchTokens
        $explicitPlan = if ($intent -eq 'pipeline') { Get-ExplicitPlanArgument -Tokens $dispatchTokens } else { $null }

        if ($xmachineContext.Requested -and -not $xmachineContext.WorkNode) {
            $availableNodes = if ($xmachineContext.AvailableNodes.Count -gt 0) { $xmachineContext.AvailableNodes -join ', ' } else { '<none configured>' }
            Write-Dispatch @{
                COMMAND = 'error'
                ACTION = "xmachine execution requires both the literal keyword 'xmachine' and a valid work-node alias from xmachine.config.json. Available aliases: $availableNodes"
            }
            break
        }

        $subcommands = @('init','research','deep-research','pipeline')
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
                'deep-research' {
                    $action = 'Activate the /gal deep-research skill for multi-source investigation with cross-review and independent reference verification.'
                    $onComplete = 'Synthesize findings, verify references, and surface RESEARCH_COMPLETE to the user.'
                }
                'pipeline' {
                    $action = 'Follow the /gal-pipeline procedure to chain implement → test → review using model-roles for multi-vendor AI assignment.'
                    $onComplete = 'Report combined verdict: implement/test/review status and whether the branch is ready for /ship.'
                }
            }

            if ($xmachineContext.Requested) {
                $action = "$action Use xmachine work node '$($xmachineContext.WorkNode)' for bounded execution where supported, and keep control-plane state convergence local."
            }

            $dispatchFields = [ordered]@{
                COMMAND = $intent
                ACTION = $action
                ON_COMPLETE = $onComplete
            }

            if ($explicitPlan) {
                $dispatchFields['PLAN'] = $explicitPlan
            }

            if ($xmachineContext.Requested) {
                $dispatchFields['READ'] = $xmachineDocPath
                $dispatchFields['EXECUTION'] = 'xmachine'
                $dispatchFields['WORK_NODE'] = $xmachineContext.WorkNode
            }

            Write-Dispatch $dispatchFields
            break
        }

        $resolved = if ($intent) { Resolve-Golem $intent } else { $null }
        if ($resolved) {
            if ($utilityGolems -contains $resolved) {
                $mode = 'utility'
            } else {
                $mode = 'consult'
            }
            $action = if ($subText) { $subText } else { "Invoke $resolved — awaiting user instruction." }
            $dispatchFields = [ordered]@{
                ROLE         = $resolved
                MODE         = $mode
                ACTION       = $action
                ON_COMPLETE  = 'Report result to user.'
            }

            if ($xmachineContext.Requested) {
                $dispatchFields['READ'] = $xmachineDocPath
                $dispatchFields['EXECUTION'] = 'xmachine'
                $dispatchFields['WORK_NODE'] = $xmachineContext.WorkNode
            }

            Write-Dispatch $dispatchFields
            break
        }

        if ($intent -and $subcommands -notcontains $intent) {
            Write-Dispatch @{
                COMMAND = 'error'
                ACTION = "Unknown argument: '$intent'. Use a subcommand (init/research/deep-research/pipeline) or a golem name."
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
                ACTION  = "Repo is initialized but no active workflow is recorded. Use /planning to create a source plan, then /refining-plan to lock the implementation contract, then /plan-to-prompt to generate the execution prompt, or /gal status for details."
                ON_COMPLETE = 'Run /gal status or /planning'
            }
            break
        }
        if ($context.Kind -eq 'state-error') {
            Write-Dispatch @{
                COMMAND = 'suggest'
                ACTION  = "Repo is initialized, but the active plan reference is invalid. Inspect .dev/state.md Active Plans and $($context.ActivePlanPath)."
                ON_COMPLETE = 'Fix repo state, then run /gal status'
            }
            break
        }
        Write-Dispatch @{
            COMMAND = 'suggest'
            ACTION  = "Active plan detected at $($context.ActivePlanPath). Use /gal whats-next to choose the next specialist command from plan files, not dispatcher state."
            ON_COMPLETE = 'Run /gal whats-next'
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
