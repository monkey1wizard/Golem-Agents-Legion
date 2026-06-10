$repoRoot = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$scriptPath = Join-Path $repoRoot 'scripts\common\New-TaskSpec.ps1'
$taskSpecDir = Join-Path $repoRoot '.dev\task-specs'

function New-TestPromptFile {
    param(
        [string]$Content
    )

    $path = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName() + '.prompt.md')
    [System.IO.File]::WriteAllText($path, $Content, [System.Text.UTF8Encoding]::new($false))
    return $path
}

Describe 'New-TaskSpec.ps1' {
    BeforeEach {
        if (Test-Path $taskSpecDir) {
            Remove-Item -LiteralPath $taskSpecDir -Recurse -Force
        }
    }

    It 'extracts a multi-line task block up to the next task boundary' {
        $promptPath = New-TestPromptFile @'
# Prompt

## Files to Create or Modify

- `[MODIFY] scripts/common/New-TaskSpec.ps1` — extractor
- `[ADD] tests/powershell/New-TaskSpec.Tests.ps1` — coverage
- `[MODIFY] docs/ignore.md` — unrelated

## Tasks

- [ ] **T-02** — extractor
  - File: `scripts/common/New-TaskSpec.ps1`
  - Change: preserve the full block and include `tests/powershell/New-TaskSpec.Tests.ps1`.
  - Acceptance: stop before the next task.
- [ ] **T-03** — another task
  - File: `docs/ignore.md`

## Test Plan

Pending.
'@

        try {
            & $scriptPath -TaskScope 'T-02' -PromptPath $promptPath -ConventionHints 'tests/fixtures/README.md' | Out-Null
            $specPath = Join-Path $taskSpecDir 'T-02-implement.md'
            $spec = Get-Content $specPath -Raw -Encoding UTF8

            $spec | Should Match '## Task Goal\s+- \[ \] \*\*T-02\*\* — extractor'
            $spec | Should Match 'File: `scripts/common/New-TaskSpec.ps1`'
            $spec | Should Match 'Change: preserve the full block and include `tests/powershell/New-TaskSpec.Tests.ps1`\.'
            $spec | Should Not Match 'T-03'
        }
        finally {
            Remove-Item -LiteralPath $promptPath -Force -ErrorAction SilentlyContinue
        }
    }

    It 'extracts a trailing task block through the end of the Tasks section' {
        $promptPath = New-TestPromptFile @'
# Prompt

## Files to Create or Modify

- `[MODIFY] scripts/common/New-TaskSpec.ps1` — extractor

## Tasks

- [ ] **T-01** — first
  - File: `docs/ignore.md`
- [ ] **T-02** — trailing
  - File: `scripts/common/New-TaskSpec.ps1`
  - Acceptance: include the trailing bullet.

## Test Plan

Pending.
'@

        try {
            & $scriptPath -TaskScope 'T-02' -PromptPath $promptPath -ConventionHints 'tests/fixtures/README.md' | Out-Null
            $spec = Get-Content (Join-Path $taskSpecDir 'T-02-implement.md') -Raw -Encoding UTF8

            $spec | Should Match 'Acceptance: include the trailing bullet\.'
            $spec | Should Not Match 'T-01\*\* — first'
        }
        finally {
            Remove-Item -LiteralPath $promptPath -Force -ErrorAction SilentlyContinue
        }
    }

    It 'extracts a single-line task item without requiring nested bullets' {
        $promptPath = New-TestPromptFile @'
# Prompt

## Files to Create or Modify

- `[MODIFY] scripts/common/New-TaskSpec.ps1` — extractor

## Tasks

- [ ] **T-02** — single-line task with `scripts/common/New-TaskSpec.ps1`

## Test Plan

Pending.
'@

        try {
            & $scriptPath -TaskScope 'T-02' -PromptPath $promptPath -ConventionHints 'tests/fixtures/README.md' | Out-Null
            $spec = Get-Content (Join-Path $taskSpecDir 'T-02-implement.md') -Raw -Encoding UTF8

            $spec | Should Match 'single-line task with `scripts/common/New-TaskSpec.ps1`'
        }
        finally {
            Remove-Item -LiteralPath $promptPath -Force -ErrorAction SilentlyContinue
        }
    }

    It 'narrows affected files to the paths named inside the task block' {
        $promptPath = New-TestPromptFile @'
# Prompt

## Files to Create or Modify

- `[MODIFY] scripts/common/New-TaskSpec.ps1` — extractor
- `[ADD] tests/powershell/New-TaskSpec.Tests.ps1` — coverage
- `[MODIFY] docs/ignore.md` — unrelated

## Tasks

- [ ] **T-02** — extractor
  - File: `scripts/common/New-TaskSpec.ps1`
  - Change: add coverage in `tests/powershell/New-TaskSpec.Tests.ps1`.

## Test Plan

Pending.
'@

        try {
            & $scriptPath -TaskScope 'T-02' -PromptPath $promptPath -ConventionHints 'tests/fixtures/README.md' | Out-Null
            $spec = Get-Content (Join-Path $taskSpecDir 'T-02-implement.md') -Raw -Encoding UTF8

            $spec | Should Match '\[MODIFY\] scripts/common/New-TaskSpec.ps1'
            $spec | Should Match '\[ADD\] tests/powershell/New-TaskSpec.Tests.ps1'
            $spec | Should Not Match 'docs/ignore.md'
        }
        finally {
            Remove-Item -LiteralPath $promptPath -Force -ErrorAction SilentlyContinue
        }
    }

    It 'falls back to the full files section when the task block names no file paths' {
        $promptPath = New-TestPromptFile @'
# Prompt

## Files to Create or Modify

- `[MODIFY] scripts/common/New-TaskSpec.ps1` — extractor
- `[MODIFY] docs/ignore.md` — unrelated

## Tasks

- [ ] **T-02** — extractor
  - Change: keep the behavior description path-free in this task body.

## Test Plan

Pending.
'@

        try {
            & $scriptPath -TaskScope 'T-02' -PromptPath $promptPath -ConventionHints 'tests/fixtures/README.md' | Out-Null
            $spec = Get-Content (Join-Path $taskSpecDir 'T-02-implement.md') -Raw -Encoding UTF8

            $spec | Should Match '\[MODIFY\] scripts/common/New-TaskSpec.ps1'
            $spec | Should Match '\[MODIFY\] docs/ignore.md'
        }
        finally {
            Remove-Item -LiteralPath $promptPath -Force -ErrorAction SilentlyContinue
        }
    }
}