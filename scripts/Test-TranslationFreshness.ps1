#requires -Version 7
<#
.SYNOPSIS
    Report translation freshness for docs/i18n/<lang>/<name>.<lang>.md against their canonical sources.

.DESCRIPTION
    For every translation under docs/i18n/, reads the front-matter (source, lang, source_commit)
    and compares source_commit against the canonical source file's latest commit
    (git log -1 --format=%H -- <source>). Reports one row per (doc, lang):

      current  source_commit is a hash that matches the source's latest commit
      stale    source_commit missing / not a hash (e.g. PENDING) / differs from the source
      missing  an allowlisted (doc, lang) pair has no translation file

    Report-only: always exits 0 unless arguments are invalid. The translation policy
    lives in docs/devguide.md#documentation-conventions; the on-location guide is
    docs/i18n/guide.md.
#>
[CmdletBinding()]
param(
    # Canonical sources that are expected to have a translation in every discovered language.
    [string[]] $Allowlist = @('README.md', 'docs/manual.md')
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
$i18nRoot = Join-Path $repoRoot 'docs/i18n'

if (-not (Test-Path $i18nRoot)) {
    Write-Host "No docs/i18n/ tree found; nothing to check."
    exit 0
}

function Get-FrontMatter {
    param([string] $Path)
    $lines = Get-Content -LiteralPath $Path
    if ($lines.Count -lt 1 -or $lines[0].Trim() -ne '---') { return $null }
    $fm = @{}
    for ($i = 1; $i -lt $lines.Count; $i++) {
        if ($lines[$i].Trim() -eq '---') { break }
        if ($lines[$i] -match '^\s*([A-Za-z_]+)\s*:\s*(.+?)\s*$') { $fm[$Matches[1]] = $Matches[2] }
    }
    return $fm
}

$rows = @()
$languages = Get-ChildItem -LiteralPath $i18nRoot -Directory | Select-Object -ExpandProperty Name

foreach ($tx in Get-ChildItem -LiteralPath $i18nRoot -Recurse -File -Filter '*.md') {
    if ($tx.Name -eq 'guide.md') { continue }  # EN-only meta signpost, not a translation
    $rel = $tx.FullName.Substring($repoRoot.Length + 1).Replace('\', '/')
    $fm = Get-FrontMatter $tx.FullName
    if ($null -eq $fm -or -not $fm.ContainsKey('source')) {
        $rows += [pscustomobject]@{ doc = '(unknown)'; lang = '?'; file = $rel; status = 'stale'; note = 'no front-matter source' }
        continue
    }
    $source = $fm['source']; $lang = $fm['lang']; $stamp = $fm['source_commit']
    $sourcePath = Join-Path $repoRoot $source
    if (-not (Test-Path $sourcePath)) {
        $rows += [pscustomobject]@{ doc = $source; lang = $lang; file = $rel; status = 'stale'; note = 'source missing' }
        continue
    }
    $actual = (git -C $repoRoot log -1 --format=%H -- $source).Trim()
    if ($stamp -match '^[0-9a-f]{7,40}$' -and ($actual.StartsWith($stamp) -or $stamp.StartsWith($actual))) {
        $rows += [pscustomobject]@{ doc = $source; lang = $lang; file = $rel; status = 'current'; note = '' }
    }
    else {
        $note = if ($stamp -notmatch '^[0-9a-f]{7,40}$') { "unstamped ($stamp)" } else { "source advanced to $($actual.Substring(0,7))" }
        $rows += [pscustomobject]@{ doc = $source; lang = $lang; file = $rel; status = 'stale'; note = $note }
    }
}

# Missing: allowlisted source has no translation for a discovered language.
foreach ($src in $Allowlist) {
    foreach ($lang in $languages) {
        $have = $rows | Where-Object { $_.doc -eq $src -and $_.lang -eq $lang }
        if (-not $have) {
            $rows += [pscustomobject]@{ doc = $src; lang = $lang; file = '(none)'; status = 'missing'; note = 'allowlisted, no translation' }
        }
    }
}

$rows | Sort-Object doc, lang | Format-Table -AutoSize
$summary = $rows | Group-Object status | ForEach-Object { "$($_.Name)=$($_.Count)" }
Write-Host ("Summary: " + ($summary -join '  '))
exit 0
