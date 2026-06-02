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
# Extract task goal from ## Tasks section
# ---------------------------------------------------------------------------
$promptContent = Get-Content $PromptPath -Raw -Encoding UTF8
$promptLines   = Get-Content $PromptPath -Encoding UTF8

$taskGoal = $promptLines | Where-Object { $_ -match "^\s*-\s*\[.?\]\s*$([regex]::Escape($TaskScope))\s*" } |
    Select-Object -First 1

if ([string]::IsNullOrWhiteSpace($taskGoal)) {
    [Console]::Error.WriteLine("New-TaskSpec.ps1: task '$TaskScope' not found in ## Tasks section of '$PromptPath'")
    exit 1
}
$taskGoal = $taskGoal.Trim() -replace '^-\s*\[.?\]\s*', ''

# ---------------------------------------------------------------------------
# Extract affected files from ## Files to Create or Modify
# ---------------------------------------------------------------------------
$inFilesSection = $false
$affectedFiles  = @()
foreach ($line in $promptLines) {
    if ($line -match '^## Files to Create or Modify') { $inFilesSection = $true; continue }
    if ($inFilesSection -and $line -match '^##\s') { break }
    if ($inFilesSection -and $line -match '^\s*-\s') {
        $affectedFiles += $line.Trim()
    }
}

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
    review    = "Write review results to ``## Review Results`` in the file: $PromptPath"
    verify    = "Write verification results to ``## Analyze`` in the file: $PromptPath"
}
$writeBackInstruction = $writeBackMap[$Phase.ToLower()]
if (-not $writeBackInstruction) { $writeBackInstruction = $writeBackMap['implement'] }

# ---------------------------------------------------------------------------
# Agent contract path per phase
# ---------------------------------------------------------------------------
$agentMap = @{
    implement = 'agent/golem-implementer.agent.md'
    test      = 'agent/golem-tester.agent.md'
    review    = 'agent/golem-reviewer.agent.md'
    verify    = 'agent/golem-verifier.agent.md'
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
