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
    Write-Host "  ask <agent>                       Describe adapter-level direct consult command"
    Write-Host "  run <agent>                       Describe adapter-level utility invocation"
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

function Require-StateFile {
    $statePath = Get-StatePath
    if (-not (Test-Path $statePath)) {
        throw "Missing .dev/state.md in current directory. Run gal init first."
    }

    return $statePath
}

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptRoot

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
        $statePath = Require-StateFile
        $stateContent = Get-Content -Path $statePath
        $interestingLines = $stateContent | Where-Object {
            $_ -match "^Workflow:" -or $_ -match "^Plan:" -or $_ -match "^Step:" -or $_ -match "^Last activity:" -or $_ -match "^Last session:" -or $_ -match "^Next step:"
        }

        Write-Host ("State file: {0}" -f $statePath)
        $interestingLines | ForEach-Object { Write-Host $_ }
        break
    }
    "next" {
        $statePath = Require-StateFile
        $nextLine = Get-Content -Path $statePath | Where-Object { $_ -match "^Next step:" } | Select-Object -First 1

        if (-not $nextLine) {
            throw "No 'Next step:' entry found in $statePath"
        }

        Write-Host $nextLine
        break
    }
    "pause" {
        $statePath = Require-StateFile
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
    "ask" {
        if (-not $Arguments -or $Arguments.Count -eq 0) {
            throw "Usage: gal ask <agent>"
        }

        Write-Host ("Adapter-level consult command: /gal ask {0}" -f ($Arguments -join " "))
        break
    }
    "run" {
        if (-not $Arguments -or $Arguments.Count -eq 0) {
            throw "Usage: gal run <agent>"
        }

        Write-Host ("Adapter-level utility command: /gal run {0}" -f ($Arguments -join " "))
        break
    }
    default {
        Show-Usage
        if ($Command) {
            exit 1
        }
    }
}