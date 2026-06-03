#Requires -Version 5.1

$ErrorActionPreference = 'Stop'

function Get-StagedEntries {
    $lines = @(git diff --cached --name-status --find-renames 2>$null)
    $entries = @()

    foreach ($line in $lines) {
        if ([string]::IsNullOrWhiteSpace($line)) {
            continue
        }

        $parts = $line -split "`t"
        if ($parts.Count -lt 2) {
            continue
        }

        $status = $parts[0]
        $path = if ($status -like 'R*' -and $parts.Count -ge 3) { $parts[2] } else { $parts[-1] }
        $top = if ($path -match '^[^/\\]+') { $matches[0].ToLowerInvariant() } else { '' }
        $segments = $path -split '[/\\]'
        $group = if ($segments.Count -ge 2) { ($segments[0] + '/' + $segments[1]).ToLowerInvariant() } else { $top }
        $extension = [IO.Path]::GetExtension($path).ToLowerInvariant()

        $entries += [pscustomobject]@{
            Status = $status
            Path = $path
            Top = $top
            Group = $group
            Extension = $extension
        }
    }

    return $entries
}

function Test-AllDocs([object[]]$Entries) {
    if (-not $Entries -or $Entries.Count -eq 0) {
        return $false
    }

    foreach ($entry in $Entries) {
        if ($entry.Extension -notin @('.md', '.mdx', '.txt', '.rst')) {
            return $false
        }
    }

    return $true
}

function Get-StagedFileCount([object[]]$Entries) {
    return @($Entries | Select-Object -ExpandProperty Path -Unique).Count
}

function Test-Rename([object[]]$Entries) {
    foreach ($entry in $Entries) {
        if ($entry.Status -like 'R*') {
            return $true
        }
    }
    return $false
}

function Get-CommitType([object[]]$Entries, [string]$LowerDiff) {
    if (Test-AllDocs $Entries) {
        return 'docs'
    }

    if ($Entries.Path -match '^\.github/workflows/' -or $Entries.Path -match '^\.github/actions/') {
        return 'ci'
    }

    if ($Entries.Path -match '(^|/)(test|tests)/') {
        return 'test'
    }

    if (Test-Rename $Entries) {
        return 'refactor'
    }

    if ($LowerDiff -match 'broken|stale|invalid|repair|fix|reasoning|<think>|code fence|plain text|corrected commit|local model|stabil') {
        return 'fix'
    }

    if ($Entries.Where({ $_.Status -like 'A*' -and $_.Top -in @('commands', 'agent', 'skills', 'workflows', 'templates') }).Count -gt 0) {
        return 'feat'
    }

    if ($Entries.Top -contains 'scripts' -or $Entries.Path -contains 'opencode.json' -or $Entries.Path -contains 'mcp.json') {
        return 'chore'
    }

    return 'refactor'
}

function Get-CommitScope([object[]]$Entries, [string]$LowerDiff) {
    if ($Entries.Path -contains 'opencode.json') {
        return 'opencode'
    }

    $commandEntries = @($Entries | Where-Object { $_.Path -match '^commands/[^/]+/' })
    if ($commandEntries.Count -gt 0) {
        $names = @($commandEntries | ForEach-Object { ($_.Path -split '/')[1].ToLowerInvariant() } | Select-Object -Unique)
        if ($names.Count -eq 1) {
            return $names[0]
        }
    }

    $skillEntries = @($Entries | Where-Object { $_.Path -match '^skills/[^/]+/' })
    if ($skillEntries.Count -gt 0) {
        $names = @($skillEntries | ForEach-Object { ($_.Path -split '/')[1].ToLowerInvariant() } | Select-Object -Unique)
        if ($names.Count -eq 1) {
            return $names[0]
        }
    }

    $dominant = $Entries |
        Group-Object Top |
        Sort-Object Count -Descending |
        Select-Object -First 1

    if ($null -eq $dominant -or [string]::IsNullOrWhiteSpace($dominant.Name)) {
        return ''
    }

    switch ($dominant.Name) {
        'agent' { return 'agents' }
        default { return $dominant.Name }
    }
}

function Get-CommitSubject([string]$Type, [string]$Scope, [object[]]$Entries, [string]$LowerDiff) {
    $hasGitCommit = ($LowerDiff -match 'git-commit-msg|git-commit|git-commits') -or ($Entries.Path -match 'git-commit-msg|git-commit|git-commits')
    $mentionsLocalModels = $LowerDiff -match 'local model|<think>|code fence|reasoning'

    if (Test-Rename $Entries -and -not [string]::IsNullOrWhiteSpace($Scope)) {
        return 'rename ' + $Scope
    }

    if ($hasGitCommit) {
        switch ($Type) {
            'feat' {
                if ($mentionsLocalModels) {
                    return 'add dynamic git-commit-msg flow for local models'
                }

                return 'add dynamic git-commit-msg command flow'
            }
            'fix' { return 'stabilize git-commit-msg output for local models' }
            'docs' { return 'document git-commit-msg trigger handling' }
            'refactor' { return 'refactor git-commit-msg generation flow' }
            'chore' { return 'update git-commit-msg command wiring' }
            default { return 'update git-commit-msg generation' }
        }
    }

    $hasScope = -not [string]::IsNullOrWhiteSpace($Scope)

    switch ($Type) {
        'docs' {
            if ($hasScope) { return 'document ' + $Scope }
            return 'document repo workflows'
        }
        'feat' {
            if ($hasScope) { return 'add ' + ($Scope + ' support') }
            return 'add repo workflow support'
        }
        'fix' {
            if ($hasScope) { return 'stabilize ' + $Scope }
            return 'stabilize repo workflow'
        }
        'refactor' {
            if ($hasScope) { return 'refactor ' + $Scope }
            return 'refactor repo workflow'
        }
        'test' {
            if ($hasScope) { return 'cover ' + $Scope }
            return 'cover repo workflow'
        }
        'ci' {
            if ($hasScope) { return 'adjust ' + $Scope }
            return 'adjust ci workflow'
        }
        default {
            if ($hasScope) { return 'update ' + $Scope }
            return 'update repo workflow'
        }
    }
}

function Get-CommitBullets([object[]]$Entries, [string]$LowerDiff) {
    $bullets = New-Object System.Collections.Generic.List[string]

    if (($Entries.Path -match '^commands/git-commit-msg/') -contains $true) {
        $bullets.Add('add a source-of-truth git-commit-msg command under commands/')
    }

    if ($Entries.Path -contains 'opencode.json') {
        $bullets.Add('route the repo OpenCode git-commit-msg command through staged helper output')
    }

    if ($LowerDiff -match '<think>|code fence|plain text|reasoning') {
        $bullets.Add('block reasoning-tag and fenced-output regressions in commit responses')
    }

    if ($bullets.Count -eq 0 -and $Entries.Count -gt 2) {
        $topGroups = @($Entries | Group-Object Top | Sort-Object Count -Descending | Select-Object -First 2)
        foreach ($group in $topGroups) {
            if (-not [string]::IsNullOrWhiteSpace($group.Name)) {
                $bullets.Add('update ' + $group.Name + ' files for staged commit generation')
            }
        }
    }

    return @($bullets | Select-Object -First 3)
}

$repoRoot = (git rev-parse --show-toplevel 2>$null | Select-Object -First 1)
if ([string]::IsNullOrWhiteSpace($repoRoot)) {
    Write-Output 'Not a git repository.'
    exit 0
}

Push-Location $repoRoot.Trim()
try {
    $entries = @(Get-StagedEntries)
    if ($entries.Count -eq 0) {
        Write-Output 'No changes staged for commit.'
        exit 0
    }

    $diffText = @(git diff --cached --no-ext-diff 2>$null) -join "`n"
    $lowerDiff = $diffText.ToLowerInvariant()
    $type = Get-CommitType $entries $lowerDiff
    $scope = Get-CommitScope $entries $lowerDiff
    $subject = Get-CommitSubject $type $scope $entries $lowerDiff

    $header = if ([string]::IsNullOrWhiteSpace($scope)) {
        "${type}: $subject"
    }
    else {
        "${type}(${scope}): $subject"
    }

    $bullets = @(Get-CommitBullets $entries $lowerDiff)
    $fileCount = Get-StagedFileCount $entries

    Write-Output $header
    if ($fileCount -gt 3 -and $bullets.Count -gt 0) {
        Write-Output ''
        foreach ($bullet in $bullets) {
            Write-Output ('- ' + $bullet)
        }
    }
}
finally {
    Pop-Location
}