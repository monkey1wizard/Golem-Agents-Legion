#Requires -Version 5.1

$ErrorActionPreference = 'Stop'

$results = [System.Collections.Generic.List[string]]::new()
$passCount = 0
$failCount = 0

function Assert-True {
    param(
        [bool]$Condition,
        [string]$Label
    )

    if ($Condition) {
        $script:passCount++
        $script:results.Add("PASS: $Label")
    }
    else {
        $script:failCount++
        $script:results.Add("FAIL: $Label")
    }
}

function Assert-Equal {
    param(
        $Expected,
        $Actual,
        [string]$Label
    )

    if ($Expected -eq $Actual) {
        $script:passCount++
        $script:results.Add("PASS: $Label")
    }
    else {
        $script:failCount++
        $script:results.Add("FAIL: $Label (expected='$Expected', actual='$Actual')")
    }
}

function Assert-Contains {
    param(
        [string]$Text,
        [string]$Needle,
        [string]$Label
    )

    Assert-True -Condition ($Text.Contains($Needle)) -Label $Label
}

function Get-DispatchFields {
    param([string[]]$Arguments)

    $galScript = Join-Path $PSScriptRoot 'gal.ps1'
    $output = & powershell -NoProfile -ExecutionPolicy Bypass -File $galScript 'dispatch' @Arguments | Out-String
    $fields = [ordered]@{}

    foreach ($line in ($output -split "`r?`n")) {
        if ($line -match '^(?<key>[A-Z_]+):\s*(?<value>.*)$') {
            $fields[$Matches['key']] = $Matches['value']
        }
    }

    return [pscustomobject]@{
        RawOutput = $output
        Fields = $fields
    }
}

function Split-SemicolonList {
    param([string]$Text)

    if ([string]::IsNullOrWhiteSpace($Text)) {
        return @()
    }

    return @($Text -split ';\s*' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
}

function Get-ByteCount {
    param([string]$Text)

    if ($null -eq $Text) {
        return 0
    }

    return [System.Text.Encoding]::UTF8.GetByteCount($Text)
}

$repoRoot = Split-Path $PSScriptRoot -Parent
$opencodePath = Join-Path $repoRoot 'plugins\gal-core\opencode.json'
$agentsPath = Join-Path $repoRoot 'AGENTS.md'
$copilotPath = Join-Path (Join-Path $repoRoot '.github') 'copilot-instructions.md'
$claudePath = Join-Path $repoRoot 'CLAUDE.md'
$geminiPath = Join-Path $repoRoot 'GEMINI.md'
$pluginRoot = Join-Path $repoRoot 'plugins\gal-core'
$pipelineSkillCandidates = @(
    (Join-Path $pluginRoot 'commands\gal-pipeline\SKILL.md'),
    (Join-Path $pluginRoot 'commands\gal-pipeline\SKILL.template.md')
)
$pipelineSkillPath = $pipelineSkillCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $pipelineSkillPath) {
    throw 'Missing gal-pipeline command contract source. Expected plugins\gal-core\commands\gal-pipeline\SKILL.md or plugins\gal-core\commands\gal-pipeline\SKILL.template.md.'
}

$baselineStartupPayloadBytes = 111207
$baselineGeneratedAdapterRereadBytes = 53923

$tempDir = Join-Path $repoRoot '.tmp\pipeline-token-burn-tests'
$goPlanPath = Join-Path $tempDir 'go-task-plan.md'
$csharpPlanPath = Join-Path $tempDir 'csharp-task-plan.md'

try {
    [System.IO.Directory]::CreateDirectory($tempDir) | Out-Null

    [System.IO.File]::WriteAllText($goPlanPath, @(
        '# Temp Go Task Plan'
        ''
        '## Tasks'
        ''
        '- [ ] T-100 Implement Go CLI command for pipeline telemetry'
        ''
    ) -join "`n", [System.Text.UTF8Encoding]::new($false))

    [System.IO.File]::WriteAllText($csharpPlanPath, @(
        '# Temp CSharp Task Plan'
        ''
        '## Tasks'
        ''
        '- [ ] T-200 Add C# verifier integration for pipeline summaries'
        ''
    ) -join "`n", [System.Text.UTF8Encoding]::new($false))

    $opencode = Get-Content $opencodePath -Raw | ConvertFrom-Json
    $instructions = @($opencode.instructions)
    Assert-Equal -Expected 1 -Actual $instructions.Count -Label 'OpenCode loads exactly one startup adapter'
    Assert-Equal -Expected 'AGENTS.md' -Actual $instructions[0] -Label 'OpenCode startup adapter is AGENTS.md'

    $startupPayloadBytes = (Get-Item (Join-Path $repoRoot $instructions[0])).Length
    Assert-True -Condition ($startupPayloadBytes -le 60000) -Label 'OpenCode startup payload is at or below 60 KB'

    $implementerContract = Get-Content (Join-Path $pluginRoot 'agents\golem-implementer.agent.md') -Raw
    $testerContract = Get-Content (Join-Path $pluginRoot 'agents\golem-tester.agent.md') -Raw
    $reviewerContract = Get-Content (Join-Path $pluginRoot 'agents\golem-reviewer.agent.md') -Raw
    $securityContract = Get-Content (Join-Path $pluginRoot 'agents\golem-security.agent.md') -Raw
    $verifierContract = Get-Content (Join-Path $pluginRoot 'agents\golem-verifier.agent.md') -Raw

    Assert-Contains -Text $implementerContract -Needle '.dev/project.md' -Label 'Implementer fallback references .dev/project.md'
    Assert-Contains -Text $testerContract -Needle '.dev/project.md' -Label 'Tester fallback references .dev/project.md'
    Assert-Contains -Text $reviewerContract -Needle '.dev/project.md' -Label 'Reviewer fallback references .dev/project.md'
    Assert-Contains -Text $securityContract -Needle '.dev/project.md' -Label 'Security fallback references .dev/project.md'
    Assert-Contains -Text $verifierContract -Needle '.dev/project.md' -Label 'Verifier fallback references .dev/project.md'

    $goDispatch = Get-DispatchFields -Arguments @('implementer', '.tmp/pipeline-token-burn-tests/go-task-plan.md', '--pipeline-phase', 'implement', '--task-scope', 'T-100')
    $goHints = Split-SemicolonList $goDispatch.Fields['CONVENTION_HINTS']
    Assert-Equal -Expected 'true' -Actual $goDispatch.Fields['CONTEXT_CARRY'] -Label 'Go task dispatch emits CONTEXT_CARRY'
    Assert-Equal -Expected 'full' -Actual $goDispatch.Fields['PIPELINE_CONTEXT_MODE'] -Label 'Implement phase uses full context mode'
    Assert-True -Condition ($goHints -contains (Join-Path $pluginRoot 'conventions\go.md')) -Label 'Go task dispatch includes go.md convention'
    Assert-True -Condition ($goHints -notcontains (Join-Path $pluginRoot 'conventions\csharp.md')) -Label 'Go task dispatch excludes csharp.md convention'
    Assert-True -Condition ($goHints -notcontains (Join-Path $pluginRoot 'conventions\rust.md')) -Label 'Go task dispatch excludes rust.md convention'
    Assert-True -Condition ($goHints -notcontains (Join-Path $pluginRoot 'conventions\typescript.md')) -Label 'Go task dispatch excludes typescript.md convention'

    $csharpDispatch = Get-DispatchFields -Arguments @('implementer', '.tmp/pipeline-token-burn-tests/csharp-task-plan.md', '--pipeline-phase', 'implement', '--task-scope', 'T-200')
    $csharpHints = Split-SemicolonList $csharpDispatch.Fields['CONVENTION_HINTS']
    Assert-True -Condition ($csharpHints -contains (Join-Path $pluginRoot 'conventions\csharp.md')) -Label 'C# task dispatch includes csharp.md convention'
    Assert-True -Condition ($csharpHints -notcontains (Join-Path $pluginRoot 'conventions\go.md')) -Label 'C# task dispatch excludes go.md convention'
    Assert-True -Condition ($csharpHints -notcontains (Join-Path $pluginRoot 'conventions\rust.md')) -Label 'C# task dispatch excludes rust.md convention'
    Assert-True -Condition ($csharpHints -notcontains (Join-Path $pluginRoot 'conventions\typescript.md')) -Label 'C# task dispatch excludes typescript.md convention'

    $boundedPipelineDispatch = Get-DispatchFields -Arguments @('pipeline', 'docs/plans/fix-gal-pipeline-token-burn.md', 'from', 'T-001', 'stop-at', 'T-001')
    Assert-Equal -Expected 'docs/plans/fix-gal-pipeline-token-burn.md' -Actual $boundedPipelineDispatch.Fields['PLAN'] -Label 'Pipeline dispatch preserves explicit plan path'
    Assert-Equal -Expected 'T-001' -Actual $boundedPipelineDispatch.Fields['FROM'] -Label 'Pipeline dispatch emits FROM boundary'
    Assert-Equal -Expected 'T-001' -Actual $boundedPipelineDispatch.Fields['STOP_AT'] -Label 'Pipeline dispatch emits STOP_AT boundary'

    $implementPlanDispatch = Get-DispatchFields -Arguments @('implementer', 'docs/plans/fix-gal-pipeline-token-burn.md', '--pipeline-phase', 'implement')
    $testDispatch = Get-DispatchFields -Arguments @('tester', 'docs/plans/fix-gal-pipeline-token-burn.md', '--pipeline-phase', 'test')
    $reviewDispatch = Get-DispatchFields -Arguments @('reviewer', 'docs/plans/fix-gal-pipeline-token-burn.md', '--pipeline-phase', 'review')
    $verifyDispatch = Get-DispatchFields -Arguments @('verifier', 'docs/plans/fix-gal-pipeline-token-burn.md', '--pipeline-phase', 'verify')
    Assert-Equal -Expected 'full' -Actual $implementPlanDispatch.Fields['PIPELINE_CONTEXT_MODE'] -Label 'Implement phase uses full context mode for source-plan dispatch'
    Assert-Equal -Expected 'true' -Actual $testDispatch.Fields['CONTEXT_CARRY'] -Label 'Test phase emits CONTEXT_CARRY=true'
    Assert-Equal -Expected 'delta' -Actual $testDispatch.Fields['PIPELINE_CONTEXT_MODE'] -Label 'Test phase uses delta context mode'
    Assert-True -Condition (-not $testDispatch.Fields.Contains('PIPELINE_CONTEXT_FILES')) -Label 'Test phase delta dispatch omits PIPELINE_CONTEXT_FILES'
    Assert-True -Condition (-not $testDispatch.Fields.Contains('CONVENTION_HINTS')) -Label 'Test phase delta dispatch omits CONVENTION_HINTS'
    Assert-Equal -Expected 'true' -Actual $reviewDispatch.Fields['CONTEXT_CARRY'] -Label 'Review phase emits CONTEXT_CARRY=true'
    Assert-Equal -Expected 'delta' -Actual $reviewDispatch.Fields['PIPELINE_CONTEXT_MODE'] -Label 'Review phase uses delta context mode'
    Assert-True -Condition (-not $reviewDispatch.Fields.Contains('PIPELINE_CONTEXT_FILES')) -Label 'Review phase delta dispatch omits PIPELINE_CONTEXT_FILES'
    Assert-True -Condition (-not $reviewDispatch.Fields.Contains('CONVENTION_HINTS')) -Label 'Review phase delta dispatch omits CONVENTION_HINTS'
    Assert-Equal -Expected 'true' -Actual $verifyDispatch.Fields['CONTEXT_CARRY'] -Label 'Verify phase emits CONTEXT_CARRY=true'
    Assert-Equal -Expected 'delta' -Actual $verifyDispatch.Fields['PIPELINE_CONTEXT_MODE'] -Label 'Verify phase uses delta context mode'
    Assert-True -Condition (-not $verifyDispatch.Fields.Contains('PIPELINE_CONTEXT_FILES')) -Label 'Verify phase delta dispatch omits PIPELINE_CONTEXT_FILES'
    Assert-True -Condition (-not $verifyDispatch.Fields.Contains('CONVENTION_HINTS')) -Label 'Verify phase delta dispatch omits CONVENTION_HINTS'

    $agentsContent = Get-Content $agentsPath -Raw
    Assert-True -Condition (-not $agentsContent.Contains('<!-- Source: conventions/csharp.md -->')) -Label 'AGENTS.md excludes csharp convention block for this repo'
    Assert-True -Condition (-not $agentsContent.Contains('<!-- Source: conventions/go.md -->')) -Label 'AGENTS.md excludes go convention block for this repo'
    Assert-True -Condition (-not $agentsContent.Contains('<!-- Source: conventions/rust.md -->')) -Label 'AGENTS.md excludes rust convention block for this repo'
    Assert-True -Condition (-not $agentsContent.Contains('<!-- Source: conventions/typescript.md -->')) -Label 'AGENTS.md excludes typescript convention block for this repo'
    Assert-Contains -Text $agentsContent -Needle '<!-- Source: conventions/conventions.md -->' -Label 'AGENTS.md keeps conventions overview block'
    Assert-Contains -Text $agentsContent -Needle '<!-- Source: conventions/token-budget.md -->' -Label 'AGENTS.md keeps token-budget block'
    Assert-Contains -Text $agentsContent -Needle '<!-- Source: conventions/working-hours.md -->' -Label 'AGENTS.md keeps working-hours block'

    $pipelineSkill = Get-Content $pipelineSkillPath -Raw
    $pipelineSkillLabel = Split-Path $pipelineSkillPath -Leaf
    Assert-Contains -Text $pipelineSkill -Needle 'Verification Independence: DEGRADED_SAME_RUNTIME' -Label "$pipelineSkillLabel carries same-runtime degraded marker"
    Assert-Contains -Text $pipelineSkill -Needle 'Verification Independence: DEGRADED_BUNDLED' -Label "$pipelineSkillLabel carries bundled degraded marker"
    Assert-Contains -Text $pipelineSkill -Needle 'No git commit that records task progress or task completion' -Label "$pipelineSkillLabel carries commit-boundary convergence gate"
    Assert-Contains -Text $pipelineSkill -Needle 'The tester writes a `### [T-NNN] YYYY-MM-DD` subsection under `## Test Results`.' -Label "$pipelineSkillLabel keeps task-scoped test write-back"
    Assert-Contains -Text $pipelineSkill -Needle 'The reviewer writes a `### [T-NNN] YYYY-MM-DD` subsection under `## Review Results`.' -Label "$pipelineSkillLabel keeps task-scoped review write-back"
    Assert-Contains -Text $pipelineSkill -Needle 'Same-runtime fallback never waives retry ceilings, protected-path escalation, conditional security review, interrupted-phase handoff, or final verifier requirements.' -Label "$pipelineSkillLabel keeps same-runtime safety guardrails"
    Assert-Contains -Text $pipelineSkill -Needle 'If `stop-at T-NNN` was specified and this task matches: **STOP**.' -Label "$pipelineSkillLabel keeps explicit stop-at boundary"
    Assert-Contains -Text $pipelineSkill -Needle 'Keep exactly one `OPEN` interrupted-phase block per task and phase.' -Label "$pipelineSkillLabel keeps interrupted-phase handoff rule"
    Assert-Contains -Text $pipelineSkill -Needle 'If `Test Retry Count` = 3: **STOP**.' -Label "$pipelineSkillLabel keeps test retry ceiling"
    Assert-Contains -Text $pipelineSkill -Needle 'If `Review Retry Count` = 3: **STOP**.' -Label "$pipelineSkillLabel keeps review retry ceiling"
    Assert-Contains -Text $pipelineSkill -Needle 'Protected Path violation, **STOP immediately** regardless of retry count.' -Label "$pipelineSkillLabel keeps protected-path escalation"
    Assert-Contains -Text $pipelineSkill -Needle 'After all unchecked tasks are complete, dispatch `golem-verifier` for a plan-level goal-backward verification pass.' -Label "$pipelineSkillLabel keeps final verifier pass"

    $implementPhaseBytes = Get-ByteCount $implementPlanDispatch.RawOutput
    $testPhaseBytes = Get-ByteCount $testDispatch.RawOutput
    $reviewPhaseBytes = Get-ByteCount $reviewDispatch.RawOutput
    $currentSingleTaskTotalBytes = $startupPayloadBytes + $implementPhaseBytes + $testPhaseBytes + $reviewPhaseBytes
    $currentFiveTaskTotalBytes = $startupPayloadBytes + (5 * ($implementPhaseBytes + $testPhaseBytes + $reviewPhaseBytes))
    $baselineSingleTaskTotalBytes = $baselineStartupPayloadBytes + (3 * $baselineGeneratedAdapterRereadBytes)
    $baselineFiveTaskTotalBytes = $baselineStartupPayloadBytes + (15 * $baselineGeneratedAdapterRereadBytes)

    Assert-True -Condition ($currentSingleTaskTotalBytes -le 80000) -Label 'Single-task 3-phase carrier load is at or below 80 KB'
    Assert-True -Condition ($currentFiveTaskTotalBytes -le 200000) -Label 'Five-task carrier load is at or below 200 KB'

    Write-Host ''
    Write-Host '=== Pipeline Token Burn Checks ==='
    Write-Host "Startup payload bytes: $startupPayloadBytes"
    Write-Host ('Adapter sizes: ' + ((Get-Item $agentsPath, $copilotPath, $claudePath, $geminiPath | ForEach-Object { '{0}={1}' -f $_.Name, $_.Length }) -join ', '))
    Write-Host ("Single-task 3-phase carrier load: before={0}, after={1}" -f $baselineSingleTaskTotalBytes, $currentSingleTaskTotalBytes)
    Write-Host ("Five-task carrier load: before={0}, after={1}" -f $baselineFiveTaskTotalBytes, $currentFiveTaskTotalBytes)
}
finally {
    if (Test-Path $goPlanPath) {
        Remove-Item $goPlanPath -Force
    }
    if (Test-Path $csharpPlanPath) {
        Remove-Item $csharpPlanPath -Force
    }
    if ((Test-Path $tempDir) -and -not (Get-ChildItem $tempDir -Force | Select-Object -First 1)) {
        Remove-Item $tempDir -Force
    }
}

Write-Host ''
Write-Host '=== Test Results ==='
Write-Host "Passed: $passCount"
Write-Host "Failed: $failCount"
Write-Host ''
foreach ($result in $results) {
    Write-Host $result
}

if ($failCount -gt 0) {
    exit 1
}