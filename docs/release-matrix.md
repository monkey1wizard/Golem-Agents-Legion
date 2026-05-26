# Release Artifact Matrix

This document defines the definitive release artifacts for the `gal` CLI (the **package-managed payload**) and how they are ingested by downstream package managers. 

It establishes the policy that all distribution channels (e.g., `winget`, `homebrew`, manual archive downloads) rely on the **same versioned single executable binary**.

## 1. Primary Artifact Lineage (The Executables)

The GitHub Releases page is the single source of truth for all built artifacts. For every official release, the following canonical single-file executables are generated:

| OS | Architecture | Artifact Name | Intended For |
| --- | --- | --- | --- |
| Windows | x64 | `gal.exe` | `winget` ingestion, manual drop-in |
| Windows | arm64 | `gal-arm64.exe` | `winget` ingestion, manual drop-in |
| macOS | x64 | `gal-darwin-x64` | `homebrew` ingestion, manual drop-in |
| macOS | arm64 | `gal-darwin-arm64` | `homebrew` ingestion, manual drop-in |
| Linux | x64 | `gal-linux-x64` | `homebrew` ingestion, manual drop-in |
| Linux | arm64 | `gal-linux-arm64` | `homebrew` ingestion, manual drop-in |

> **Note**: macOS and Linux binaries are ultimately installed simply as `gal` in the target `bin/` directory by the respective package manager or user.

## 2. Fallback Archives (Manual Downloads)

To support manual installation and locked enterprise environments that cannot use `winget` or `homebrew`, we provide fallback archives. 

**Critical Constraint**: These archives must *only* contain the exact same executable from the primary lineage, plus the `LICENSE` file. They must **never** include a second copy of configuration files, `.dev/` structures, or `catalog.json`.

| Target | Archive Artifact | Contents |
| --- | --- | --- |
| Windows (x64) | `gal-windows-x64.zip` | `gal.exe`, `LICENSE` |
| Windows (arm64) | `gal-windows-arm64.zip` | `gal-arm64.exe`, `LICENSE` |
| macOS (x64) | `gal-darwin-x64.tar.gz` | `gal`, `LICENSE` |
| macOS (arm64) | `gal-darwin-arm64.tar.gz` | `gal`, `LICENSE` |
| Linux (x64) | `gal-linux-x64.tar.gz` | `gal`, `LICENSE` |
| Linux (arm64) | `gal-linux-arm64.tar.gz` | `gal`, `LICENSE` |

## 3. Metadata, Provenance, and Security

Every release must include security and ingestion metadata to guarantee the integrity of the binary.

| File | Purpose |
| --- | --- |
| `checksums.txt` | SHA-256 hashes for all `.exe`, binary files, `.zip`, and `.tar.gz` artifacts. Used by users and package managers to verify downloads. |
| `checksums.txt.sig` | Optional: GPG or sigstore signature of the `checksums.txt` file to establish provenance. |
| `LICENSE` | MIT License document included inside archives and alongside the source. |

## 4. Package Manager Ingestion Maps

Package managers are strictly downstream consumers of the GitHub Releases page. They do not build from source.

### Winget (Windows)
- **Source**: `winget-pkgs` repository manifest.
- **Payload**: Points directly to `gal.exe` or `gal-windows-x64.zip` (depending on packaging preferences in `winget`, typically the `.zip` or a standalone installer).
- **Validation**: Winget manifest includes the SHA-256 hash derived from `checksums.txt`.

### Homebrew (macOS / Linux)
- **Source**: Homebrew tap or core formula.
- **Payload**: Points directly to the `.tar.gz` archives (`gal-darwin-x64.tar.gz`, etc.).
- **Validation**: The `rb` formula specifies the SHA-256 hash derived from `checksums.txt`.
- **Installation**: Extracts the archive and moves the `gal` binary to `/usr/local/bin/` (or equivalent brew prefix).

## 5. What is Explicitly Excluded

To maintain strict ownership boundaries (as defined in the Developer Guide), release artifacts **must not** contain:
- `~/.gal/` directory structures or presets.
- Default `config.json` files.
- Pre-resolved plugin lockfiles.
- Any repository-local state (`.dev/` or `docs/`). 

The CLI binary handles bootstrapping these structures on first execution if they do not exist.
