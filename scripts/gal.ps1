# ENTRY SWITCH (T-011, BUG-B): The Rust `gal` binary is now the single entry
# for `install`, `update`, `uninstall`, `doctor`, and `commit-msg`.
# This script handles the developer workflow only (init, dispatch-to-golems,
# xmachine). The frozen scripts remain as oracle only and are NOT invoked for
# install/update/uninstall by any entry path.
#
# To install/update GAL, use the Rust binary: gal install / gal update

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
    Write-Host "  pipeline <TaskSpecPath> [role]      Orchestrate task execution via executor routing"
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

function Get-SourcePlanPath([string]$ExecutionPlanPath) {
    if ([string]::IsNullOrWhiteSpace($ExecutionPlanPath)) { return $null }

    $repoContextRoot = Get-RepoContextRoot
    $devPlansRoot = Join-Path $repoContextRoot '.dev\plans'
    if (-not $ExecutionPlanPath.StartsWith($devPlansRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        return $null
    }

    $relativePromptPath = $ExecutionPlanPath.Substring($devPlansRoot.Length).TrimStart('\\')
    if ($relativePromptPath -notmatch '\.prompt\.md$') {
        return $null
    }

    $relativeSourcePath = $relativePromptPath -replace '\.prompt\.md$', '.md'
    return Join-Path (Join-Path $repoContextRoot 'docs\plans') $relativeSourcePath
}

function Get-LanguageConventionFileNames([string]$SignalText) {
    $fileNames = [System.Collections.Generic.List[string]]::new()
    $languageMap = [ordered]@{
        'c#' = 'csharp.md'
        '.net' = 'csharp.md'
        'typescript' = 'typescript.md'
        'javascript' = 'typescript.md'
        'go' = 'go.md'
        'golang' = 'go.md'
        'rust' = 'rust.md'
    }

    if ([string]::IsNullOrWhiteSpace($SignalText)) {
        return @()
    }

    $normalizedSignalText = $SignalText.ToLowerInvariant()
    foreach ($key in $languageMap.Keys) {
        if ($normalizedSignalText.Contains($key) -and $fileNames -notcontains $languageMap[$key]) {
            $fileNames.Add($languageMap[$key])
        }
    }

    return @($fileNames)
}

function Get-TaskScopeLanguageSignalText {
    param(
        [string]$TaskScope,
        [string[]]$PlanPaths
    )

    if ([string]::IsNullOrWhiteSpace($TaskScope)) {
        return $null
    }

    foreach ($planPath in $PlanPaths | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -Unique) {
        if (-not (Test-Path $planPath)) {
            continue
        }

        foreach ($line in Get-Content $planPath) {
            if ($line -match [regex]::Escape($TaskScope)) {
                return $line.Trim()
            }
        }
    }

    return $null
}

function Get-ProjectLanguageConventionPaths {
    param(
        [string]$TaskScope,
        [string[]]$PlanPaths
    )

    $repoContextRoot = Get-RepoContextRoot
    $projectPath = Join-Path $repoContextRoot '.dev\project.md'
    $conventionsRoot = Join-Path $repoContextRoot 'conventions'
    $paths = [System.Collections.Generic.List[string]]::new()

    foreach ($path in @(
        (Join-Path $conventionsRoot 'token-budget.md'),
        (Join-Path $conventionsRoot 'working-hours.md')
    )) {
        if ((Test-Path $path) -and $paths -notcontains $path) {
            $paths.Add($path)
        }
    }

    foreach ($fileName in (Get-LanguageConventionFileNames -SignalText (Get-TaskScopeLanguageSignalText -TaskScope $TaskScope -PlanPaths $PlanPaths))) {
        $path = Join-Path $conventionsRoot $fileName
        if ((Test-Path $path) -and $paths -notcontains $path) {
            $paths.Add($path)
        }
    }

    if ($paths.Count -gt 2) {
        return @($paths)
    }

    if (-not (Test-Path $projectPath)) {
        return @($paths)
    }

    $languageLine = Get-Content $projectPath | Where-Object { $_ -match '^\|\s*Language\s*\|' } | Select-Object -First 1
    if ([string]::IsNullOrWhiteSpace($languageLine)) {
        return @($paths)
    }

    foreach ($fileName in (Get-LanguageConventionFileNames -SignalText $languageLine)) {
        $path = Join-Path $conventionsRoot $fileName
        if ((Test-Path $path) -and $paths -notcontains $path) {
            $paths.Add($path)
        }
    }

    return @($paths)
}

function Get-PipelineDispatchMetadata {
    param(
        [string]$Phase,
        [bool]$ContextCarrySupported,
        [string]$PreferredPlanPath,
        [string]$TaskScope
    )

    $stateContext = Get-StateContext
    $activePlanPath = $stateContext.ActivePlanPath
    $sourcePlanPath = Get-SourcePlanPath -ExecutionPlanPath $activePlanPath

    if (-not [string]::IsNullOrWhiteSpace($PreferredPlanPath)) {
        $resolvedPlanPath = Resolve-PlanPath $PreferredPlanPath
        if ($resolvedPlanPath -match '\.prompt\.md$') {
            $activePlanPath = $resolvedPlanPath
            $sourcePlanPath = Get-SourcePlanPath -ExecutionPlanPath $resolvedPlanPath
        } elseif ($resolvedPlanPath -match '\.md$') {
            $activePlanPath = $null
            $sourcePlanPath = $resolvedPlanPath
        }
    }

    $contextMode = if ($ContextCarrySupported -and $Phase -ne 'implement') { 'delta' } else { 'full' }
    $metadata = [ordered]@{
        CURRENT_TASK = if ($activePlanPath) { Get-PlanStatusField -PlanPath $activePlanPath -FieldName 'Current Task' } else { $null }
        TASK_BASE_COMMIT = if ($activePlanPath) { Get-PlanStatusField -PlanPath $activePlanPath -FieldName 'Task Base Commit' } else { $null }
        TASK_FINAL_COMMIT = if ($activePlanPath) { Get-PlanStatusField -PlanPath $activePlanPath -FieldName 'Task Final Commit' } else { $null }
        TEST_RETRY_COUNT = if ($activePlanPath) { Get-PlanStatusField -PlanPath $activePlanPath -FieldName 'Test Retry Count' } else { $null }
        REVIEW_RETRY_COUNT = if ($activePlanPath) { Get-PlanStatusField -PlanPath $activePlanPath -FieldName 'Review Retry Count' } else { $null }
        CONTEXT_CARRY = if ($ContextCarrySupported) { 'true' } else { 'false' }
        PIPELINE_CONTEXT_MODE = $contextMode
    }

    if ($contextMode -eq 'full') {
        $metadata['ACTIVE_EXECUTION_PROMPT'] = $activePlanPath
        $metadata['SOURCE_PLAN'] = $sourcePlanPath
        $metadata['WORKFLOW_STATE'] = $stateContext.WorkflowState
        $metadata['STATUS_STEP'] = if ($activePlanPath) { Get-PlanStatusField -PlanPath $activePlanPath -FieldName 'Step' } else { $null }
        $metadata['STATUS_LAST_ACTIVITY'] = if ($activePlanPath) { Get-PlanStatusField -PlanPath $activePlanPath -FieldName 'Last activity' } else { $null }
        $metadata['STATUS_NEXT_STEP'] = if ($activePlanPath) { Get-PlanStatusField -PlanPath $activePlanPath -FieldName 'Next step' } else { $null }
        $metadata['PIPELINE_CONTEXT_FILES'] = ((@(
            (Join-Path (Get-RepoContextRoot) '.dev\project.md'),
            (Join-Path (Get-RepoContextRoot) '.dev\state.md'),
            $activePlanPath,
            $sourcePlanPath
        ) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -Unique) -join '; ')
        $metadata['CONVENTION_HINTS'] = (((Get-ProjectLanguageConventionPaths -TaskScope $TaskScope -PlanPaths @($activePlanPath, $sourcePlanPath) | Select-Object -Unique)) -join '; ')
    }

    return $metadata
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
. (Join-Path $scriptRoot 'common\Common.ps1')

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
    $configPath = (Resolve-XmachineConfigRecord -RepoRoot $repoRoot).Path
    if (-not (Test-Path $configPath)) {
        return @()
    }

    try {
        $config = Get-Content $configPath -Raw | ConvertFrom-Json
    }
    catch {
        throw "Invalid JSON in '$configPath'. $($_.Exception.Message)"
    }

    if ($null -eq $config) {
        return @()
    }

    $aliasProperty = if ($config.PSObject.Properties.Name.Contains('xmachineNodeAliases')) {
        'xmachineNodeAliases'
    } elseif ($config.PSObject.Properties.Name.Contains('nodes')) {
        'nodes'
    } else {
        $null
    }

    if ($null -eq $aliasProperty) {
        return @()
    }

    $nodes = $config.$aliasProperty
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

function Test-IsExplicitPlanToken([string]$Token) {
    if ([string]::IsNullOrWhiteSpace($Token)) {
        return $false
    }

    $candidate = $Token.Trim()
    if ($candidate.StartsWith('#file:', [System.StringComparison]::OrdinalIgnoreCase)) {
        $candidate = $candidate.Substring(6)
    }

    return $candidate -match '\.(prompt\.md|md)$'
}

function Get-PipelineDispatchContext {
    param([string[]]$Tokens)

    $remainingTokens = [System.Collections.Generic.List[string]]::new()
    $requested = $false
    $phase = $null
    $taskScope = $null
    $fromTask = $null
    $stopAtTask = $null
    $fixMode = $false

    for ($index = 0; $index -lt $Tokens.Count; $index++) {
        $token = $Tokens[$index]
        if ([string]::IsNullOrWhiteSpace($token)) {
            continue
        }

        switch ($token) {
            '--pipeline-phase' {
                $requested = $true
                if ($index + 1 -ge $Tokens.Count -or [string]::IsNullOrWhiteSpace($Tokens[$index + 1])) {
                    return [pscustomobject]@{
                        Requested = $false
                        Phase = $null
                        TaskScope = $null
                        FixMode = $false
                        RemainingTokens = @()
                        Error = 'Missing phase after --pipeline-phase. Expected one of: implement, test, review, verify, security.'
                    }
                }

                $index++
                $phase = $Tokens[$index].Trim().ToLowerInvariant()
                continue
            }
            '--task-scope' {
                if ($index + 1 -ge $Tokens.Count -or [string]::IsNullOrWhiteSpace($Tokens[$index + 1])) {
                    return [pscustomobject]@{
                        Requested = $false
                        Phase = $null
                        TaskScope = $null
                        FixMode = $false
                        RemainingTokens = @()
                        Error = 'Missing task reference after --task-scope.'
                    }
                }

                $index++
                $taskScope = $Tokens[$index].Trim()
                continue
            }
            '--fix-mode' {
                $fixMode = $true
                continue
            }
            'from' {
                if ($index + 1 -ge $Tokens.Count -or [string]::IsNullOrWhiteSpace($Tokens[$index + 1])) {
                    return [pscustomobject]@{
                        Requested = $false
                        Phase = $null
                        TaskScope = $null
                        From = $null
                        StopAt = $null
                        FixMode = $false
                        RemainingTokens = @()
                        Error = 'Missing task reference after from.'
                    }
                }

                $index++
                $fromTask = $Tokens[$index].Trim()
                continue
            }
            'stop-at' {
                if ($index + 1 -ge $Tokens.Count -or [string]::IsNullOrWhiteSpace($Tokens[$index + 1])) {
                    return [pscustomobject]@{
                        Requested = $false
                        Phase = $null
                        TaskScope = $null
                        From = $null
                        StopAt = $null
                        FixMode = $false
                        RemainingTokens = @()
                        Error = 'Missing task reference after stop-at.'
                    }
                }

                $index++
                $stopAtTask = $Tokens[$index].Trim()
                continue
            }
            default {
                if (Test-IsExplicitPlanToken -Token $token) {
                    continue
                }
                $remainingTokens.Add($token)
            }
        }
    }

    if ($requested) {
        $validPhases = @('implement','test','review','verify','security')
        if ([string]::IsNullOrWhiteSpace($phase)) {
            return [pscustomobject]@{
                Requested = $false
                Phase = $null
                TaskScope = $null
                From = $null
                StopAt = $null
                FixMode = $false
                RemainingTokens = @()
                Error = 'Pipeline-bound dispatch requires --pipeline-phase <implement|test|review|verify|security>.'
            }
        }

        if ($validPhases -notcontains $phase) {
            return [pscustomobject]@{
                Requested = $false
                Phase = $null
                TaskScope = $null
                From = $null
                StopAt = $null
                FixMode = $false
                RemainingTokens = @()
                Error = "Unsupported pipeline phase '$phase'. Expected one of: implement, test, review, verify, security."
            }
        }
    }

    return [pscustomobject]@{
        Requested = $requested
        Phase = $phase
        TaskScope = $taskScope
        From = $fromTask
        StopAt = $stopAtTask
        FixMode = $fixMode
        RemainingTokens = @($remainingTokens)
        Error = $null
    }
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
               'golem-notewriter','golem-designer','golem-researcher',
               'golem-security','golem-releaser')
    # accept with or without 'golem-' prefix
    $full = if ($Name -like 'golem-*') { $Name } else { "golem-$Name" }
    if ($known -contains $full) { return $full }
    return $null
}

$utilityGolems = @('golem-debugger','golem-notewriter')

switch ($Command) {
    "init" {
        & gal init-repo @Arguments
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
            ACTION = "Follow the /gal-pipeline procedure only to resolve task '$($shorthand.TaskRef)' and prepare the bounded task spec, then offload with scripts/Invoke-XmachineTask.ps1 -WorkNode '$($shorthand.WorkNode)' -TaskSpec <phase-task-spec> -Wait. Do not use Invoke-XmachinePipeline.* for this shorthand. Do not pass -WorkRepoPath unless this target repo has an intentional persistent checkout on the work node."
            ON_COMPLETE = 'Report the single-task verdict and whether local convergence is complete.'
            READ = $xmachineDocPath
            EXECUTION = 'xmachine'
            OFFLOAD = 'direct-task'
            XMACHINE_MODE = 'execute'
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
    "pipeline" {
        # T-009: Thin shim — delegates to the gal-dispatch Rust bin.
        # Usage: gal pipeline <TaskSpecPath> [--phase <phase>] [--task <T-NNN>] [--receipt <path>]
        # Exit codes mirror gal-dispatch: 0=completed, 1=no-receipt/failed, 2=unavailable/degraded
        #
        # When the gal-dispatch bin is absent (not yet distributed to this machine), this shim
        # outputs the minimal text dispatch so callers are never silently broken.

        $workDir     = Get-RepoContextRoot
        $binName     = 'gal-dispatch'
        $binFound    = $null -ne (Get-Command $binName -ErrorAction SilentlyContinue)

        if (-not $binFound) {
            # Bin absent — output minimal text dispatch (backward-compatible degradation)
            Write-Output '--- GAL DISPATCH ---'
            Write-Output "Dispatch: reason=bin-absent executor=text-dispatch"
            Write-Error "gal-dispatch bin not found in PATH; text dispatch fallback active. Install the bin or add it to PATH." -ErrorAction Continue
            exit 2
        }

        # Parse args: --phase, --task, --receipt, spec path
        $phase       = 'implement'
        $taskId      = ''
        $receiptPath = ''
        $specPath    = ''
        $i = 0
        while ($i -lt $Arguments.Count) {
            switch ($Arguments[$i]) {
                '--phase'   { $i++; $phase       = $Arguments[$i] }
                '--task'    { $i++; $taskId      = $Arguments[$i] }
                '--receipt' { $i++; $receiptPath = $Arguments[$i] }
                default     { if (-not $specPath) { $specPath = $Arguments[$i] } }
            }
            $i++
        }

        if (-not $specPath) {
            Write-Error "Usage: gal pipeline <TaskSpecPath> [--phase <phase>] [--task <T-NNN>] [--receipt <path>]" -ErrorAction Stop
        }
        if (-not (Test-Path $specPath)) {
            Write-Error "Task spec not found: $specPath" -ErrorAction Stop
        }

        # Auto-derive task id from spec filename (e.g. T-007-implement.md → T-007)
        if (-not $taskId) {
            $baseName = [System.IO.Path]::GetFileNameWithoutExtension($specPath)
            if ($baseName -match '^(T-\d+)') { $taskId = $Matches[1] }
        }
        if (-not $taskId) { $taskId = 'T-unknown' }

        $binArgs = @('--phase', $phase, '--task', $taskId, '--workdir', $workDir)
        if ($receiptPath) { $binArgs += @('--receipt', $receiptPath) }

        # Pipe spec content to bin stdin
        Get-Content $specPath -Raw -Encoding UTF8 | & $binName @binArgs
        exit $LASTEXITCODE
    }
    "dispatch" {
        $intent  = if ($Arguments.Count -gt 0) { $Arguments[0] } else { '' }
        $dispatchTokens = if ($Arguments.Count -gt 1) { $Arguments[1..($Arguments.Count-1)] } else { @() }
        $xmachineContext = Get-XmachineDispatchContext -Tokens $dispatchTokens
        $explicitPlan = Get-ExplicitPlanArgument -Tokens $dispatchTokens
        $pipelineContext = Get-PipelineDispatchContext -Tokens $dispatchTokens

        if ($xmachineContext.Requested -and -not $xmachineContext.WorkNode) {
            $availableNodes = if ($xmachineContext.AvailableNodes.Count -gt 0) { $xmachineContext.AvailableNodes -join ', ' } else { '<none configured>' }
            Write-Dispatch @{
                COMMAND = 'error'
                ACTION = "xmachine execution requires both the literal keyword 'xmachine' and a valid work-node alias from ~/.gal/config/xmachine.json (legacy repo-root fallback supported). Available aliases: $availableNodes"
            }
            break
        }

        if ($pipelineContext.Error) {
            Write-Dispatch @{
                COMMAND = 'error'
                ACTION = $pipelineContext.Error
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
                    $action = 'Follow the /gal-pipeline procedure to chain implement → test → review using executor-routing.json for multi-vendor AI assignment.'
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

            if (-not [string]::IsNullOrWhiteSpace($pipelineContext.From)) {
                $dispatchFields['FROM'] = $pipelineContext.From
            }

            if (-not [string]::IsNullOrWhiteSpace($pipelineContext.StopAt)) {
                $dispatchFields['STOP_AT'] = $pipelineContext.StopAt
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
            $isPipelineGolem = @('golem-implementer','golem-tester','golem-reviewer','golem-verifier','golem-security') -contains $resolved

            if ($utilityGolems -contains $resolved) {
                $mode = 'utility'
            } elseif ($pipelineContext.Requested -and $isPipelineGolem) {
                $mode = 'bound'
            } else {
                $mode = 'consult'
            }

            $pipelineAction = if ($pipelineContext.RemainingTokens.Count -gt 0) { $pipelineContext.RemainingTokens -join ' ' } else { $null }
            $action = if ($pipelineContext.Requested) {
                if ($pipelineAction) { $pipelineAction } else { "Invoke $resolved for pipeline phase '$($pipelineContext.Phase)'." }
            } elseif ($dispatchTokens.Count -gt 0) {
                $dispatchTokens -join ' '
            } else {
                "Invoke $resolved — awaiting user instruction."
            }

            $dispatchFields = [ordered]@{
                ROLE         = $resolved
                MODE         = $mode
                ACTION       = $action
                ON_COMPLETE  = 'Report result to user.'
            }

            if ($pipelineContext.Requested -and $isPipelineGolem) {
                $dispatchFields['DISPATCH_KIND'] = 'pipeline-phase'
                $dispatchFields['PIPELINE_PHASE'] = $pipelineContext.Phase
                if ($pipelineContext.TaskScope) {
                    $dispatchFields['TASK_SCOPE'] = $pipelineContext.TaskScope
                }
                if ($pipelineContext.FixMode) {
                    $dispatchFields['FIX_MODE'] = 'true'
                }

                $contextCarrySupported = -not $xmachineContext.Requested
                foreach ($entry in (Get-PipelineDispatchMetadata -Phase $pipelineContext.Phase -ContextCarrySupported:$contextCarrySupported -PreferredPlanPath $explicitPlan -TaskScope $pipelineContext.TaskScope).GetEnumerator()) {
                    if ($null -ne $entry.Value -and $entry.Value -ne '') {
                        $dispatchFields[$entry.Key] = $entry.Value
                    }
                }
            }

            if ($xmachineContext.Requested) {
                $dispatchFields['READ'] = $xmachineDocPath
                $dispatchFields['EXECUTION'] = 'xmachine'
                $dispatchFields['WORK_NODE'] = $xmachineContext.WorkNode
            }

            # --- Headless executor OFFLOAD (T-008) ---
            # When executor routing resolves for the current phase+role, emit an OFFLOAD block
            # instead of the regular text dispatch. Dispatch itself does not spawn the process.
            # Falls through to regular Write-Dispatch when routing is absent or spec generation fails.
            if ($pipelineContext.Requested -and -not $xmachineContext.Requested -and $pipelineContext.TaskScope) {
                $phaseRole  = Get-PipelinePhaseRole -Phase $pipelineContext.Phase
                $routing    = Read-ExecutorRouting
                $routeEntry = if ($routing -and $phaseRole) { $routing[$phaseRole] } else { $null }
                $executor   = if ($routeEntry) { $routeEntry.executor } else { $null }
                $model      = if ($routeEntry) { $routeEntry.model }    else { $null }

                if (-not [string]::IsNullOrWhiteSpace($executor)) {
                    $promptPath = $dispatchFields['ACTIVE_EXECUTION_PROMPT']
                    if ([string]::IsNullOrWhiteSpace($promptPath)) {
                        $promptPath = (Get-StateContext).ActivePlanPath
                    }
                    $conventionHints = $dispatchFields['CONVENTION_HINTS']

                    $specScriptPath = Join-Path $scriptRoot 'common\New-TaskSpec.ps1'
                    $specArgs = @('-TaskScope', $pipelineContext.TaskScope, '-Phase', $pipelineContext.Phase)
                    if (-not [string]::IsNullOrWhiteSpace($promptPath)) {
                        $specArgs += @('-PromptPath', $promptPath)
                    }
                    if (-not [string]::IsNullOrWhiteSpace($conventionHints)) {
                        $specArgs += @('-ConventionHints', $conventionHints)
                    }

                    $specPath = $null
                    try {
                        $specPath = & pwsh -NonInteractive -File $specScriptPath @specArgs 2>$null |
                            Select-Object -Last 1
                    } catch { $specPath = $null }

                    if (-not [string]::IsNullOrWhiteSpace($specPath) -and (Test-Path $specPath)) {
                        # T-011: gal-dispatch bin replaced Invoke-Executor.ps1
                        Write-Dispatch ([ordered]@{
                            COMMAND         = 'offload'
                            OFFLOAD         = 'headless-executor'
                            DISPATCH_MODE   = 'offload'
                            EXECUTOR        = $executor
                            MODEL           = if ($model) { $model } else { '(default)' }
                            ROLE            = $phaseRole
                            PIPELINE_PHASE  = $pipelineContext.Phase
                            TASK_SCOPE      = $pipelineContext.TaskScope
                            TASK_SPEC       = $specPath
                            ACTION          = "Run: Get-Content '$specPath' -Raw | gal-dispatch --phase $($pipelineContext.Phase) --task $($pipelineContext.TaskScope) --workdir '$repoRoot'. The gal-dispatch bin reads routing, spawns the executor, and outputs a Dispatch: marker line. Exit 0 → completed with write-back verified. Exit 1 → no-receipt or disconnected-partial. Exit 2 → unavailable/degraded. Fall back to role-playing the $phaseRole golem only on exit 2."
                            ON_COMPLETE     = "Record in execution prompt the Dispatch: marker line emitted by gal-dispatch. Example: Dispatch: phase=$($pipelineContext.Phase) task=$($pipelineContext.TaskScope) role=$phaseRole executor=$executor model=$(if ($model) { $model } else { 'default' }) state=<state> session_id=<id> log=<path>"
                            BYPASS_PERMISSION_WARNING = 'SECURITY: headless executor runs with --dangerously-skip-permissions or --allow-all. Full trust of secondary CLI filesystem access. Enable only in a trusted local environment.'
                        })
                        break
                    }
                }
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
