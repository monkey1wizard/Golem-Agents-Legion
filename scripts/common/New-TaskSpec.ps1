#Requires -Version 7
<#
.SYNOPSIS
  Generate a compact, self-contained Task Spec for a T-NNN task.

.DESCRIPTION
  Assembles a Markdown spec (< 5KB) from the active execution prompt so a
  secondary headless CLI has just enough context to act on one task without
  reading the full codebase.

  Content of the spec:
    - Task goal (the matching T-NNN line from ## Tasks)
    - Affected files (filtered from ## Files to Create or Modify)
    - Git branch and HEAD commit
    - Write-back path instruction (which file / section to write results to)
    - Language-convention hints (reused from Get-PipelineDispatchMetadata via
      gal.ps1 dispatch output; no parallel detection logic)
    - Agent contract path
    - Explicit prohibition on git commit / push

  Output: .dev/task-specs/<TaskScope>-<Phase>.md (treated as transient; not
  retained after pipeline completion — see -Cleanup).

.PARAMETER TaskScope
  Task identifier, e.g. "T-001".

.PARAMETER Phase
  Pipeline phase: implement | test | review | verify. Default: implement.

.PARAMETER PromptPath
  Path to the active execution prompt (.dev/plans/<slug>.prompt.md).
  Auto-detected from .dev/state.md when omitted.

.PARAMETER ConventionHints
  Semicolon-separated paths to language-convention files.
  When omitted, calls gal.ps1 dispatch to extract the value (same mechanism
  as Get-PipelineDispatchMetadata CONVENTION_HINTS — no parallel logic).

.PARAMETER Cleanup
  Switch. Remove the entire .dev/task-specs/ directory instead of generating
  a spec. Use at pipeline end to discard transient artifacts.
#>
param(
    [string]$TaskScope,
    [string]$Phase = 'implement',
    [string]$PromptPath,
    [string]$ConventionHints,
    [switch]$Cleanup
)

$ErrorActionPreference = 'Stop'

function Get-MarkdownSectionLines {
    param(
        [string[]]$Lines,
        [string]$Header
    )

    $inSection = $false
    $sectionLines = @()

    foreach ($line in $Lines) {
        if (-not $inSection) {
            if ($line -match "^##\s+$([regex]::Escape($Header))\s*$") {
                $inSection = $true
            }
            continue
        }

        if ($line -match '^##\s+') {
            break
        }

        $sectionLines += $line
    }

    return ,$sectionLines
}

function Get-TaskBlockLines {
    param(
        [string[]]$PromptLines,
        [string]$TaskId
    )

    $taskSectionLines = Get-MarkdownSectionLines -Lines $PromptLines -Header 'Tasks'
    if ($taskSectionLines.Count -eq 0) {
        return @()
    }

    # Match markdown task list items with optional bold wrappers around the task id.
    $taskStartPattern = "^\s*-\s*\[.?\]\s*(?:\*\*)?$([regex]::Escape($TaskId))(?:\*\*)?(?:\b|\s|\()"
    # The next top-level task bullet ends the current task block; indented bullets stay inside.
    $nextTaskPattern = '^\s*-\s*\[.?\]\s*(?:\*\*)?T-\d+'

    $startIndex = -1
    for ($index = 0; $index -lt $taskSectionLines.Count; $index++) {
        if ($taskSectionLines[$index] -match $taskStartPattern) {
            $startIndex = $index
            break
        }
    }

    if ($startIndex -lt 0) {
        return @()
    }

    $taskBlockLines = @()
    for ($index = $startIndex; $index -lt $taskSectionLines.Count; $index++) {
        $line = $taskSectionLines[$index]
        if ($index -gt $startIndex -and $line -match $nextTaskPattern) {
            break
        }

        $taskBlockLines += $line
    }

    return ,$taskBlockLines
}

function Get-AffectedFileLines {
    param(
        [string[]]$PromptLines,
        [string[]]$TaskBlockLines
    )

    $filesSectionLines = Get-MarkdownSectionLines -Lines $PromptLines -Header 'Files to Create or Modify'
    $fallbackFileLines = @($filesSectionLines | Where-Object { $_ -match '^\s*-\s' } | ForEach-Object { $_.Trim() })

    if ($TaskBlockLines.Count -eq 0) {
        return ,$fallbackFileLines
    }

    $taskText = $TaskBlockLines -join "`n"
    $pathMatches = [regex]::Matches($taskText, '`([^`\r\n]+)`')
    $orderedPaths = New-Object System.Collections.Generic.List[string]
    $seenPaths = New-Object 'System.Collections.Generic.HashSet[string]' ([System.StringComparer]::OrdinalIgnoreCase)

    foreach ($match in $pathMatches) {
        $candidate = $match.Groups[1].Value.Trim()
        if ([string]::IsNullOrWhiteSpace($candidate)) {
            continue
        }

        if ($candidate -notmatch '[/\\]') {
            continue
        }

        if ($seenPaths.Add($candidate)) {
            [void]$orderedPaths.Add($candidate)
        }
    }

    if ($orderedPaths.Count -eq 0) {
        return ,$fallbackFileLines
    }

    $selectedFileLines = New-Object System.Collections.Generic.List[string]
    foreach ($path in $orderedPaths) {
        $matchingFallbackLine = $fallbackFileLines | Where-Object { $_ -match [regex]::Escape($path) } | Select-Object -First 1
        if (-not [string]::IsNullOrWhiteSpace($matchingFallbackLine)) {
            [void]$selectedFileLines.Add($matchingFallbackLine)
        }
        else {
            [void]$selectedFileLines.Add("- ``$path``")
        }
    }

    return ,$selectedFileLines
}

# ---------------------------------------------------------------------------
# Cleanup mode — remove transient task-spec artifacts at pipeline end
# ---------------------------------------------------------------------------
if ($Cleanup) {
    $repoRoot = (Split-Path (Split-Path $PSScriptRoot -Parent) -Parent)
    $taskSpecDir = Join-Path $repoRoot '.dev\task-specs'
    if (Test-Path $taskSpecDir) {
        Remove-Item -LiteralPath $taskSpecDir -Recurse -Force
        Write-Host "[OK] Removed transient task-spec directory: $taskSpecDir"
    }
    exit 0
}

if ([string]::IsNullOrWhiteSpace($TaskScope)) {
    [Console]::Error.WriteLine('New-TaskSpec.ps1: -TaskScope is required when not using -Cleanup')
    exit 1
}

# ---------------------------------------------------------------------------
# Locate repo root and execution prompt
# ---------------------------------------------------------------------------
$repoRoot = (Split-Path (Split-Path $PSScriptRoot -Parent) -Parent)

if ([string]::IsNullOrWhiteSpace($PromptPath)) {
    $statePath = Join-Path $repoRoot '.dev\state.md'
    if (-not (Test-Path $statePath)) {
        [Console]::Error.WriteLine('New-TaskSpec.ps1: .dev/state.md not found — cannot auto-detect active prompt')
        exit 1
    }
    # Look for the headless-cli-pipeline prompt (active plan row)
    $stateLines = Get-Content $statePath -Encoding UTF8
    $activePlanMatch = $stateLines | Where-Object { $_ -match '\.prompt\.md' } | Select-Object -First 1
    if ($activePlanMatch -match '`?([^`\|]+\.prompt\.md)`?') {
        $relPath = $Matches[1].Trim()
        $PromptPath = if ([System.IO.Path]::IsPathRooted($relPath)) { $relPath }
                      else { Join-Path $repoRoot $relPath }
    }
}

if ([string]::IsNullOrWhiteSpace($PromptPath) -or -not (Test-Path $PromptPath)) {
    [Console]::Error.WriteLine("New-TaskSpec.ps1: execution prompt not found at '$PromptPath'")
    exit 1
}

# ---------------------------------------------------------------------------
# Extract the full task block from ## Tasks, not just the first task line.
# ---------------------------------------------------------------------------
$promptLines = Get-Content $PromptPath -Encoding UTF8
$taskGoalLines = Get-TaskBlockLines -PromptLines $promptLines -TaskId $TaskScope

if ($taskGoalLines.Count -eq 0) {
    [Console]::Error.WriteLine("New-TaskSpec.ps1: task '$TaskScope' not found in ## Tasks section of '$PromptPath'")
    exit 1
}
$taskGoal = ($taskGoalLines -join "`n").Trim()

# ---------------------------------------------------------------------------
# Extract task-scoped affected files when the task block names them.
# ---------------------------------------------------------------------------
$affectedFiles = Get-AffectedFileLines -PromptLines $promptLines -TaskBlockLines $taskGoalLines

# ---------------------------------------------------------------------------
# Git context
# ---------------------------------------------------------------------------
$gitBranch = (git rev-parse --abbrev-ref HEAD 2>$null).Trim()
$gitHead   = (git rev-parse --short HEAD 2>$null).Trim()

# ---------------------------------------------------------------------------
# Write-back instruction per phase
# ---------------------------------------------------------------------------
$writeBackMap = @{
    implement = "Write implementation code changes to the files listed in 'Affected Files'. Do NOT modify any other files."
    test      = "Write test results to ``## Test Results`` in the file: $PromptPath"
    audit     = "Write audit results to ``## Review Results`` in the file: $PromptPath"
    verify    = "Write verification results to ``## Analyze`` in the file: $PromptPath"
}
$writeBackInstruction = $writeBackMap[$Phase.ToLower()]
if (-not $writeBackInstruction) { $writeBackInstruction = $writeBackMap['implement'] }

# ---------------------------------------------------------------------------
# Agent contract path per phase
# ---------------------------------------------------------------------------
$agentMap = @{
    implement = 'plugins/gal-core/agents/golem-implementer.agent.md'
    test      = 'plugins/gal-core/agents/golem-tester.agent.md'
    audit     = 'plugins/gal-core/agents/golem-auditor.agent.md'
    verify    = 'plugins/gal-core/agents/golem-verifier.agent.md'
}
$agentContract = Join-Path $repoRoot ($agentMap[$Phase.ToLower()] ?? $agentMap['implement'])
$agentContractDisplay = $agentMap[$Phase.ToLower()] ?? $agentMap['implement']

# ---------------------------------------------------------------------------
# Convention hints — reuse gal.ps1 Get-PipelineDispatchMetadata (no parallel logic)
# ---------------------------------------------------------------------------
if ([string]::IsNullOrWhiteSpace($ConventionHints)) {
    $galScript = Join-Path $repoRoot 'scripts\gal.ps1'
    if (Test-Path $galScript) {
        try {
            $dispatchOutput = & pwsh -NonInteractive -File $galScript dispatch golem-implementer `
                --pipeline-phase implement --task-scope $TaskScope 2>$null | Out-String
            if ($dispatchOutput -match 'CONVENTION_HINTS:\s*(.+)') {
                $ConventionHints = $Matches[1].Trim()
            }
        }
        catch { $ConventionHints = '' }
    }
}

# ---------------------------------------------------------------------------
# Assemble spec
# ---------------------------------------------------------------------------
$affectedFilesBlock = if ($affectedFiles.Count -gt 0) {
    ($affectedFiles -join "`n")
} else {
    '(none specified)'
}

$conventionBlock = if (-not [string]::IsNullOrWhiteSpace($ConventionHints)) {
    @"
## Convention Files

Read these files for coding conventions that apply to this task:

$($ConventionHints -split ';' | ForEach-Object { "- $($_.Trim())" } | Where-Object { $_ -ne '- ' } | Out-String)
"@
} else { '' }

$agentContractBlock = if (Test-Path $agentContract) {
    "## Agent Contract`n`nFollow the instructions in: ``$agentContractDisplay``"
} else { '' }

$spec = @"
# Task Spec: $TaskScope ($Phase)

Generated: $((Get-Date).ToString('yyyy-MM-dd HH:mm'))
Git branch: $gitBranch  |  HEAD: $gitHead
Source prompt: $PromptPath

---

## Task Goal

$taskGoal

## Affected Files

$affectedFilesBlock

## Write-Back Instruction

$writeBackInstruction

$conventionBlock
$agentContractBlock

## IMPORTANT: Commit Boundary

**DO NOT run `git commit` or `git push`.**
The orchestrator holds the commit boundary.
Write changes to files only; the calling pipeline will commit after verifying write-back.

---
*This spec is transient — discard after use.*
"@

# ---------------------------------------------------------------------------
# Write spec to .dev/task-specs/
# ---------------------------------------------------------------------------
$taskSpecDir = Join-Path $repoRoot '.dev\task-specs'
if (-not (Test-Path $taskSpecDir)) {
    New-Item -ItemType Directory -Path $taskSpecDir -Force | Out-Null
}

$outFile = Join-Path $taskSpecDir "$TaskScope-$($Phase.ToLower()).md"
[System.IO.File]::WriteAllText($outFile, $spec, [System.Text.UTF8Encoding]::new($false))

# Size check
$sizeBytes = (Get-Item $outFile).Length
$sizeKB    = [math]::Round($sizeBytes / 1024, 2)

Write-Host "[OK] Task spec written: $outFile ($sizeKB KB)"
if ($sizeKB -gt 5) {
    Write-Warning "Task spec exceeds 5KB target ($sizeKB KB) — consider trimming context."
}

return $outFile
