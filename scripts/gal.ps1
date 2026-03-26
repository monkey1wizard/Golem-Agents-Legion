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
    Write-Host "  plan <name>                       Create a draft plan file from template"
    Write-Host "  status                            Show current workflow status from .dev/state.md"
    Write-Host "  next                              Show the next step from .dev/state.md"
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
        if (-not $Arguments -or $Arguments.Count -eq 0) {
            throw "Usage: gal plan <name>"
        }

        $planName = ($Arguments -join " ").Trim()
        $slug = ConvertTo-Slug $planName
        $plansDir = Join-Path (Get-Location).Path "docs\plans"
        $planTemplatePath = Join-Path $repoRoot "templates\plan.md"
        $planTargetPath = Join-Path $plansDir ("plan-{0}.prompt.md" -f $slug)

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