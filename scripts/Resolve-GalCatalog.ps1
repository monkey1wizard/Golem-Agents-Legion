#Requires -Version 5.1

<#
.SYNOPSIS
    GAL catalog resolver: deterministically resolves a plugin set from catalog + user config.

.DESCRIPTION
    Reads plugins/catalog.json and ~/.gal/config/config.json, validates entries,
    applies profile policy, and emits a deterministic resolved plugin set plus
    plugins.lock.json.

    Default profile resolves to gal-core only. Companion plugins are opt-in via
    named profiles or explicit plugin selection.
#>

[CmdletBinding()]
param(
    [string]$CatalogPath = (Join-Path $PSScriptRoot '..' 'plugins' 'catalog.json'),
    [string]$ConfigPath = (Join-Path $env:USERPROFILE '.gal' 'config' 'config.json'),
    [string]$LockfilePath = (Join-Path $env:USERPROFILE '.gal' 'state' 'plugins.lock.json'),
    [switch]$DryRun,
    [switch]$PassThru
)

function ConvertTo-Hashtable {
    param([object]$InputObject)
    if ($InputObject -eq $null) { return $null }
    if ($InputObject -is [System.Collections.IDictionary]) {
        $hash = [ordered]@{}
        foreach ($key in $InputObject.Keys) {
            $hash[$key] = ConvertTo-Hashtable -InputObject $InputObject[$key]
        }
        return $hash
    }
    if ($InputObject -is [System.Collections.IEnumerable] -and $InputObject -isnot [string]) {
        $array = [System.Collections.Generic.List[object]]::new()
        foreach ($item in $InputObject) {
            $array.Add((ConvertTo-Hashtable -InputObject $item))
        }
        return $array.ToArray()
    }
    if ($InputObject -is [System.Management.Automation.PSCustomObject]) {
        $hash = [ordered]@{}
        foreach ($prop in $InputObject.PSObject.Properties) {
            $hash[$prop.Name] = ConvertTo-Hashtable -InputObject $prop.Value
        }
        return $hash
    }
    return $InputObject
}

function Read-JsonFile {
    param([string]$Path)
    if (-not (Test-Path $Path)) { return $null }
    $json = Get-Content $Path -Raw | ConvertFrom-Json
    ConvertTo-Hashtable -InputObject $json
}

function Write-JsonFile {
    param([string]$Path, [object]$Data)
    $dir = Split-Path $Path -Parent
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    $Data | ConvertTo-Json -Depth 20 | Set-Content $Path -Encoding UTF8
}

function Test-CatalogEntry {
    param([hashtable]$Entry, [System.Collections.Generic.List[string]]$Errors)

    $requiredFields = @('pluginId','displayName','supportTier','sourceType','upstream','license','checksumPolicy','componentMap','supportedProviders','installStrategy','defaultProfiles','allowAutoUpdate','localOverridePolicy')
    foreach ($field in $requiredFields) {
        if (-not $Entry.Contains($field)) {
            $Errors.Add("[$($Entry.pluginId)] Missing required field: $field")
        }
    }

    if ($Entry.sourceType -in @('curated-upstream','mirrored','forked')) {
        if (-not $Entry.upstream -or -not $Entry.upstream.Contains('repo')) {
            $Errors.Add("[$($Entry.pluginId)] Upstream repo is required for sourceType=$($Entry.sourceType)")
        }
        if ([string]::IsNullOrWhiteSpace($Entry.license)) {
            $Errors.Add("[$($Entry.pluginId)] License is required for sourceType=$($Entry.sourceType)")
        }
        if ([string]::IsNullOrWhiteSpace($Entry.checksumPolicy)) {
            $Errors.Add("[$($Entry.pluginId)] checksumPolicy is required for sourceType=$($Entry.sourceType)")
        }
        if (-not $Entry.upstream.Contains('ref')) {
            $Errors.Add("[$($Entry.pluginId)] Upstream ref is required for sourceType=$($Entry.sourceType)")
        }
    }

    $validTiers = @('official-gal','curated-upstream','mirrored','forked','local')
    if ($Entry.supportTier -notin $validTiers) {
        $Errors.Add("[$($Entry.pluginId)] Invalid supportTier: $($Entry.supportTier). Must be one of: $($validTiers -join ', ')")
    }
}

function Resolve-PluginSet {
    param(
        [hashtable]$Catalog,
        [hashtable]$Config
    )

    $errors = [System.Collections.Generic.List[string]]::new()
    $catalogPlugins = @($Catalog.plugins)
    $catalogProfiles = $Catalog.profiles

    # --- Validate catalog entries ---
    foreach ($plugin in $catalogPlugins) {
        Test-CatalogEntry -Entry $plugin -Errors $errors
    }

    # --- Determine active profile ---
    $profileName = if ($Config -and $Config.Contains('defaultProfile')) { $Config.defaultProfile } else { 'default' }
    if (-not $catalogProfiles.Contains($profileName)) {
        $errors.Add("Profile '$profileName' not found in catalog. Available: $($catalogProfiles.Keys -join ', ')")
        $profileName = 'default'
    }

    $profile = $catalogProfiles[$profileName]
    $resolvedPluginIds = [System.Collections.Generic.List[string]]::new()

    # --- Profile plugins (always included) ---
    foreach ($pluginId in $profile.plugins) {
        if ($resolvedPluginIds -notcontains $pluginId) {
            $resolvedPluginIds.Add($pluginId)
        }
    }

    # --- Explicit enabled plugins from config ---
    if ($Config -and $Config.Contains('enabledPlugins')) {
        foreach ($pluginId in $Config.enabledPlugins) {
            if ($resolvedPluginIds -notcontains $pluginId) {
                $resolvedPluginIds.Add($pluginId)
            }
        }
    }

    # --- Explicit disabled plugins from config ---
    if ($Config -and $Config.Contains('disabledPlugins')) {
        foreach ($pluginId in $Config.disabledPlugins) {
            if ($resolvedPluginIds -contains $pluginId) {
                $resolvedPluginIds.Remove($pluginId)
            }
        }
    }

    # --- Resolve plugin metadata ---
    $resolvedPlugins = [System.Collections.Generic.List[hashtable]]::new()
    foreach ($pluginId in $resolvedPluginIds) {
        $catalogEntry = $catalogPlugins | Where-Object { $_.pluginId -eq $pluginId } | Select-Object -First 1
        if (-not $catalogEntry) {
            $errors.Add("Resolved plugin '$pluginId' not found in catalog")
            continue
        }
        $resolvedPlugins.Add([ordered]@{
            pluginId = $catalogEntry.pluginId
            displayName = $catalogEntry.displayName
            supportTier = $catalogEntry.supportTier
            sourceType = $catalogEntry.sourceType
            upstream = $catalogEntry.upstream
            license = $catalogEntry.license
            checksumPolicy = $catalogEntry.checksumPolicy
            componentMap = $catalogEntry.componentMap
            supportedProviders = $catalogEntry.supportedProviders
            installStrategy = $catalogEntry.installStrategy
            resolvedByProfile = ($profile.plugins -contains $pluginId)
            resolvedByExplicit = ($Config.enabledPlugins -contains $pluginId)
        })
    }

    # --- Drift detection placeholder: compare with existing lockfile ---
    $driftDetected = $false
    $driftDetails = [System.Collections.Generic.List[string]]::new()
    if (Test-Path $LockfilePath) {
        $existingLock = Read-JsonFile -Path $LockfilePath
        if ($existingLock -and $existingLock.Contains('resolvedPlugins')) {
            $existingIds = $existingLock.resolvedPlugins | ForEach-Object { $_.pluginId }
            $newIds = $resolvedPlugins | ForEach-Object { $_.pluginId }
            if (Compare-Object $existingIds $newIds) {
                $driftDetected = $true
                $driftDetails.Add("Resolved plugin set changed: was [$($existingIds -join ', ')], now [$($newIds -join ', ')]")
            }
        }
    }

    return [ordered]@{
        errors = @($errors)
        profileName = $profileName
        resolvedPlugins = @($resolvedPlugins)
        driftDetected = $driftDetected
        driftDetails = @($driftDetails)
    }
}

function Build-Lockfile {
    param(
        [hashtable]$Catalog,
        [hashtable]$Config,
        [array]$ResolvedPlugins,
        [string]$ProfileName
    )

    $lockfile = [ordered]@{
        schemaVersion = 1
        generatedAt = (Get-Date -Format 'o')
        resolverVersion = 'gal-resolver-1.0'
        profileName = $ProfileName
        catalogHash = ''
        resolvedPlugins = [System.Collections.Generic.List[hashtable]]::new()
        driftDetection = [ordered]@{
            lastResolvedAt = (Get-Date -Format 'o')
            catalogHash = ''
        }
    }

    # Simple catalog hash: SHA256 of canonical JSON
    $catalogJson = $Catalog | ConvertTo-Json -Depth 20 -Compress
    $lockfile.catalogHash = (Get-FileHash -InputStream ([System.IO.MemoryStream]::new([System.Text.Encoding]::UTF8.GetBytes($catalogJson))) -Algorithm SHA256).Hash
    $lockfile.driftDetection.catalogHash = $lockfile.catalogHash

    foreach ($plugin in $ResolvedPlugins) {
        $entry = [ordered]@{
            pluginId = $plugin.pluginId
            displayName = $plugin.displayName
            supportTier = $plugin.supportTier
            sourceType = $plugin.sourceType
            resolvedSource = if ($plugin.upstream) { $plugin.upstream['repo'] } else { $null }
            resolvedRef = if ($plugin.upstream) { $plugin.upstream['ref'] } else { $null }
            resolvedVersion = $null
            resolvedChecksum = $null
            resolvedLicense = $plugin.license
            resolvedComponentMap = $plugin.componentMap
            selectedProviders = $plugin.supportedProviders
            installStrategy = $plugin.installStrategy
            resolvedByProfile = $plugin.resolvedByProfile
            resolvedByExplicit = $plugin.resolvedByExplicit
        }
        $lockfile.resolvedPlugins.Add($entry)
    }

    return $lockfile
}

# --- Main ---

$Catalog = Read-JsonFile -Path $CatalogPath
if (-not $Catalog) {
    throw "Catalog not found at: $CatalogPath"
}

$Config = Read-JsonFile -Path $ConfigPath
if (-not $Config) {
    Write-Verbose "Config not found at $ConfigPath; using empty defaults"
    $Config = @{}
}

$resolution = Resolve-PluginSet -Catalog $Catalog -Config $Config

if ($resolution.errors.Count -gt 0) {
    Write-Error "Catalog validation failed with $($resolution.errors.Count) error(s):"
    foreach ($err in $resolution.errors) { Write-Error "  - $err" }
    if (-not $DryRun) { exit 1 }
}

$lockfile = Build-Lockfile -Catalog $Catalog -Config $Config -ResolvedPlugins $resolution.resolvedPlugins -ProfileName $resolution.profileName

if ($DryRun) {
    Write-Host "--- DRY RUN ---"
    Write-Host "Profile: $($resolution.profileName)"
    Write-Host "Resolved plugins: $(($resolution.resolvedPlugins | ForEach-Object { $_.pluginId }) -join ', ')"
    Write-Host "Validation errors: $($resolution.errors.Count)"
    Write-Host "Drift detected: $($resolution.driftDetected)"
    if ($resolution.driftDetails.Count -gt 0) {
        foreach ($d in $resolution.driftDetails) { Write-Host "  Drift: $d" }
    }
    Write-Host "Lockfile preview:"
    $lockfile | ConvertTo-Json -Depth 20
}
else {
    Write-JsonFile -Path $LockfilePath -Data $lockfile
    Write-Host "Lockfile written: $LockfilePath"
    Write-Host "Profile: $($resolution.profileName)"
    Write-Host "Resolved plugins: $(($resolution.resolvedPlugins | ForEach-Object { $_.pluginId }) -join ', ')"
}

if ($PassThru) {
    return [pscustomobject]@{
        ProfileName = $resolution.profileName
        ResolvedPlugins = $resolution.resolvedPlugins
        Errors = $resolution.errors
        DriftDetected = $resolution.driftDetected
        DriftDetails = $resolution.driftDetails
        Lockfile = $lockfile
    }
}
