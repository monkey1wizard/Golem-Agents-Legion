# Release Artifact Matrix

This document defines the canonical release lineage for the `gal` CLI package-managed payload and the downstream publication contract for `winget`, Homebrew, and manual fallback archives.

GitHub Releases is the single source of truth. Every supported install channel must consume the same versioned single-binary lineage, and every fallback archive must wrap the exact canonical binary for that platform plus `LICENSE` only.

## 1. Canonical Asset Naming

Release tag placeholder: `<version>` means the exact GitHub Release tag, for example `v1.2.3`.

For every release, publish these canonical binary assets directly on GitHub Releases:

| OS | Architecture | Canonical binary asset | Supported install command | Downstream consumer |
| --- | --- | --- | --- | --- |
| Windows | x64 | `gal-<version>-windows-x64.exe` | `winget install Monkey1Wizard.GAL` | `winget`, manual download |
| Windows | arm64 | `gal-<version>-windows-arm64.exe` | `winget install Monkey1Wizard.GAL` | `winget`, manual download |
| macOS | x64 | `gal-<version>-darwin-x64` | `brew install monkey1wizard/tap/gal` | Homebrew, manual download |
| macOS | arm64 | `gal-<version>-darwin-arm64` | `brew install monkey1wizard/tap/gal` | Homebrew, manual download |
| Linux | x64 | `gal-<version>-linux-x64` | `brew install monkey1wizard/tap/gal` | Homebrew, manual download |
| Linux | arm64 | `gal-<version>-linux-arm64` | `brew install monkey1wizard/tap/gal` | Homebrew, manual download |

Rules:

1. The canonical binary asset name must include the exact release tag and target platform/architecture.
2. Manual fallback archives must contain the same canonical binary filename shown above, not a renamed generic `gal` placeholder.
3. Package managers may rename the installed file to `gal` in the target `bin/` directory during installation, but they may not invent a second binary lineage.

## 2. Fallback Archive Contract

Fallback archives exist for manual install and locked-down environments that cannot use `winget` or Homebrew.

Archive contents are strictly limited to the platform's canonical binary asset plus `LICENSE`.

| Target | Archive asset | Required contents |
| --- | --- | --- |
| Windows x64 | `gal-<version>-windows-x64.zip` | `gal-<version>-windows-x64.exe`, `LICENSE` |
| Windows arm64 | `gal-<version>-windows-arm64.zip` | `gal-<version>-windows-arm64.exe`, `LICENSE` |
| macOS x64 | `gal-<version>-darwin-x64.tar.gz` | `gal-<version>-darwin-x64`, `LICENSE` |
| macOS arm64 | `gal-<version>-darwin-arm64.tar.gz` | `gal-<version>-darwin-arm64`, `LICENSE` |
| Linux x64 | `gal-<version>-linux-x64.tar.gz` | `gal-<version>-linux-x64`, `LICENSE` |
| Linux arm64 | `gal-<version>-linux-arm64.tar.gz` | `gal-<version>-linux-arm64`, `LICENSE` |

Excluded from all archives:

- `~/.gal/` directory structures or presets
- default `config.json` files
- pre-resolved plugin lockfiles
- repository-local state such as `.dev/`, `docs/`, `commands/`, or `skills/`

The bootstrap binary owns first-run initialization of `~/.gal/`; release artifacts do not pre-seed that runtime state.

## 3. Required Release Metadata and Provenance

Every canonical release must publish the following companion files:

| File | Requirement | Purpose |
| --- | --- | --- |
| `checksums.txt` | required | SHA-256 hashes for every canonical binary and fallback archive asset |
| `checksums.txt.sig` | required | Signature over `checksums.txt` using the project's chosen signing system (GPG or sigstore) |
| `artifact-manifest.json` | required | Machine-readable mapping of release tag, asset names, sha256 values, platform, architecture, and source build provenance |
| `LICENSE` | required | MIT license text included alongside artifacts and inside fallback archives |

`artifact-manifest.json` must record, at minimum:

- `version`
- `publishedAt`
- `assets[]`
- `assets[].name`
- `assets[].platform`
- `assets[].architecture`
- `assets[].sha256`
- `assets[].kind` with `binary` or `archive`
- `assets[].contains` for archive assets

## 4. Package Manager Publication Spec

Package managers are downstream consumers of GitHub Releases. They do not build from source and they do not own `~/.gal/`.

### Winget publication spec

Canonical publication target:

| Field | Required value or rule |
| --- | --- |
| `PackageIdentifier` | `Monkey1Wizard.GAL` |
| `PackageName` | `GAL` |
| `Moniker` | `gal` |
| `Publisher` | `monkey1wizard` |
| `License` | `MIT` |
| `PackageVersion` | exact GitHub Release tag without rewriting semantics |
| `Commands` | must expose `gal` |
| `Installers` | one row per supported Windows architecture |
| `InstallerUrl` | must point to the matching GitHub Release archive asset |
| `InstallerSha256` | must match `checksums.txt` for that archive asset |

Submission rules:

1. `winget` must consume `gal-<version>-windows-x64.zip` and `gal-<version>-windows-arm64.zip` from GitHub Releases.
2. The manifest must not trigger a source build, download a repo snapshot, or inject bootstrap state into `~/.gal/`.
3. If `winget` review lags the GitHub Release, the manifest description must keep GitHub Releases as the canonical fallback.

### Homebrew publication spec

Canonical publication target:

| Field | Required value or rule |
| --- | --- |
| Tap | `monkey1wizard/tap` |
| Formula name | `gal` |
| `desc` | must describe GAL as the bootstrap CLI |
| `homepage` | GitHub repository URL |
| `version` | exact GitHub Release tag |
| `license` | `MIT` |
| `url` | platform-specific GitHub Release archive asset |
| `sha256` | must match `checksums.txt` for that archive asset |
| `def install` | must install the canonical archived binary as `bin/gal` |
| `test do` | must execute `gal --version` or equivalent lightweight version check |

Submission rules:

1. Homebrew must consume the `.tar.gz` archive matching the current platform and architecture.
2. The formula must rename the archived canonical binary to `gal` only at install time via `bin.install`.
3. The formula must not synthesize extra files under `~/.gal/`, and it must not treat generated runtime state as part of the package payload.

## 5. Release Governance and Drift Policy

### Canonical version source

The GitHub Releases page for `monkey1wizard/golem-agents-legion` is the sole canonical version source. All package-manager manifests and provider-marketplace entries are downstream wrappers over that same lineage.

### Publish order

1. Build the canonical binaries and fallback archives.
2. Publish the GitHub Release with binaries, archives, `checksums.txt`, `checksums.txt.sig`, and `artifact-manifest.json`.
3. Update `winget` and Homebrew manifests to the new release assets and hashes.
4. Update provider-marketplace metadata or wrappers after package-manager publication is queued.

### Lag tolerance

Accepted downstream lag windows:

| Channel | Target window from GitHub Release publication | Escalation point |
| --- | --- | --- |
| `winget` | within 1 business day | more than 3 business days behind canonical release |
| Homebrew | within 1 business day | more than 3 business days behind canonical release |
| Provider marketplaces | best effort within 3 business days | more than 5 business days behind canonical release |

GAL does not guarantee same-day parity across every downstream channel. If a downstream channel is outside its target window, GitHub Releases remains the official fallback and the lag must be called out in channel copy.

### Verification commands

Run these checks for every release:

| Check | Command | Expected result |
| --- | --- | --- |
| Windows package metadata | `winget show --id Monkey1Wizard.GAL --exact` | reported version matches the current canonical release tag |
| Homebrew metadata | `brew info monkey1wizard/tap/gal` | reported version matches the current canonical release tag |
| PowerShell packaging dry run | `pwsh -File scripts/Package-ReleaseArtifacts.ps1 -SourceBinary <path> -Version <version> -TargetPlatform windows -TargetArch x64 -OutputDir <dir>` | emits versioned archive and binary names matching this matrix |
| POSIX packaging dry run | `bash scripts/package-release-artifacts.sh --source-binary <path> --version <version> --target-platform linux --target-arch x64 --output-dir <dir>` | emits versioned archive and binary names matching this matrix |
| Release hash audit | compare `checksums.txt` with `artifact-manifest.json` | every published asset appears once with the same SHA-256 |

### Fallback messaging

Every downstream channel that may lag must carry this guidance verbatim or with only minor style edits:

> If this channel is behind the latest canonical GAL release because of review or publish latency, install the newest version directly from GitHub Releases.
