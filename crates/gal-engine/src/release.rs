//! Release artifact generation for `gal release`.
//!
//! Produces `checksums.txt` and `artifact-manifest.json` as defined in
//! `docs/architecture.md` and `CONTRIBUTING.md`. In local dry-run mode the
//! module uses the current executable as the only asset; in CI the workflow
//! feeds in the cross-compiled binaries and archives.
//!
//! Cosign keyless signing is a CI-only operation (needs OIDC token).
//! Locally, `run_release` writes a placeholder note instead of a fake `.sig`.
//!
//! Generates package-manager release artifacts (winget + homebrew).

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors that can occur during release artifact generation.
#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Asset not found: {0}")]
    AssetNotFound(PathBuf),
    #[error("Cannot determine executable path: {0}")]
    ExePath(String),
}

/// Whether an asset is a bare binary or a fallback archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactKind {
    Binary,
    Archive,
}

/// One entry in `artifact-manifest.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactEntry {
    pub name: String,
    pub platform: String,
    pub architecture: String,
    pub sha256: String,
    pub kind: ArtifactKind,
    /// For archives: the list of files inside. Empty for bare binaries.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub contains: Vec<String>,
}

/// The full `artifact-manifest.json` structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactManifest {
    pub version: String,
    pub published_at: String,
    pub assets: Vec<ArtifactEntry>,
}

/// Options for a local release dry-run.
pub struct ReleaseOptions {
    /// The release version tag (e.g. `"v0.1.0"`).
    pub version: String,
    /// Directory to write `checksums.txt` and `artifact-manifest.json` into.
    pub output_dir: PathBuf,
    /// Assets to include. Each path must exist on disk.
    /// If empty, uses the current executable (local dev dry-run).
    pub assets: Vec<AssetSpec>,
}

/// Specification for one asset in a dry-run or CI invocation.
pub struct AssetSpec {
    /// Asset name as it would appear in the GitHub Release
    /// (e.g. `"gal-v0.1.0-windows-x64.exe"`).
    pub name: String,
    pub platform: String,
    pub architecture: String,
    pub kind: ArtifactKind,
    /// Files contained inside the archive (empty for bare binaries).
    pub contains: Vec<String>,
    /// Path to the file on disk.
    pub path: PathBuf,
}

/// Outputs produced by a release run.
pub struct ReleaseArtifacts {
    /// Path to `checksums.txt`.
    pub checksums_path: PathBuf,
    /// Path to `artifact-manifest.json`.
    pub manifest_path: PathBuf,
    /// The manifest data.
    pub manifest: ArtifactManifest,
    /// True when running locally (no OIDC → no `.sig` written).
    pub cosign_skipped: bool,
}

/// Compute the SHA-256 hex digest of a file.
pub fn sha256_file(path: &Path) -> Result<String, ReleaseError> {
    if !path.exists() {
        return Err(ReleaseError::AssetNotFound(path.to_path_buf()));
    }
    let bytes = fs::read(path)?;
    let digest = Sha256::digest(&bytes);
    Ok(hex::encode(digest))
}

/// Build `checksums.txt` content: one `sha256  name` line per entry.
/// Format matches the sha256sum(1) / Get-FileHash convention used by GitHub Releases.
/// Lines are sorted by asset name (not by hash) for easy lookup.
pub fn format_checksums_txt(entries: &[ArtifactEntry]) -> String {
    let mut sorted: Vec<&ArtifactEntry> = entries.iter().collect();
    sorted.sort_by_key(|e| e.name.as_str());
    let lines: Vec<String> = sorted
        .iter()
        .map(|e| format!("{}  {}", e.sha256, e.name))
        .collect();
    lines.join("\n") + "\n"
}

/// Return the canonical asset base name for a binary on the given platform.
///
/// Examples:
/// - `("v0.1.0", "windows", "x64")` → `"gal-v0.1.0-windows-x64.exe"`
/// - `("v0.1.0", "darwin",  "arm64")` → `"gal-v0.1.0-darwin-arm64"`
pub fn canonical_binary_name(version: &str, platform: &str, arch: &str) -> String {
    let ext = if platform == "windows" { ".exe" } else { "" };
    format!("gal-{}-{}-{}{}", version, platform, arch, ext)
}

/// Return the canonical archive name for the given platform.
pub fn canonical_archive_name(version: &str, platform: &str, arch: &str) -> String {
    let ext = if platform == "windows" {
        ".zip"
    } else {
        ".tar.gz"
    };
    format!("gal-{}-{}-{}{}", version, platform, arch, ext)
}

/// Full release asset matrix for a given version (6 binaries + 6 archives = 12).
/// Used in CI and for validation; also useful as the canonical spec reference.
pub fn release_asset_matrix(version: &str) -> Vec<(String, String, String, ArtifactKind)> {
    let platforms = [
        ("windows", "x64"),
        ("windows", "arm64"),
        ("darwin", "x64"),
        ("darwin", "arm64"),
        ("linux", "x64"),
        ("linux", "arm64"),
    ];
    let mut out = Vec::new();
    for (platform, arch) in &platforms {
        out.push((
            canonical_binary_name(version, platform, arch),
            platform.to_string(),
            arch.to_string(),
            ArtifactKind::Binary,
        ));
        out.push((
            canonical_archive_name(version, platform, arch),
            platform.to_string(),
            arch.to_string(),
            ArtifactKind::Archive,
        ));
    }
    out
}

/// Run a local release dry-run.
///
/// If `opts.assets` is empty, uses the current executable as a stand-in for
/// the Windows x64 binary (dev-machine dry-run). The resulting manifest has
/// accurate sha256 for the local binary but does NOT represent a real
/// cross-platform release.
///
/// Cosign signing is skipped locally (no OIDC token available). A
/// `checksums.txt.sig-placeholder` file is written instead to make the absence
/// explicit — no fake `.sig` is ever generated.
pub fn run_release(opts: ReleaseOptions) -> Result<ReleaseArtifacts, ReleaseError> {
    fs::create_dir_all(&opts.output_dir)?;

    // Resolve assets: use provided list, or fall back to local exe (dry-run).
    let assets: Vec<AssetSpec> = if opts.assets.is_empty() {
        let exe = std::env::current_exe().map_err(|e| ReleaseError::ExePath(e.to_string()))?;
        let name = canonical_binary_name(&opts.version, "windows", "x64");
        vec![AssetSpec {
            name,
            platform: "windows".to_string(),
            architecture: "x64".to_string(),
            kind: ArtifactKind::Binary,
            contains: vec![],
            path: exe,
        }]
    } else {
        opts.assets
    };

    // Compute SHA-256 for each asset.
    let mut entries: Vec<ArtifactEntry> = Vec::new();
    for spec in &assets {
        let sha256 = sha256_file(&spec.path)?;
        entries.push(ArtifactEntry {
            name: spec.name.clone(),
            platform: spec.platform.clone(),
            architecture: spec.architecture.clone(),
            sha256,
            kind: spec.kind.clone(),
            contains: spec.contains.clone(),
        });
    }

    // Build manifest.
    let manifest = ArtifactManifest {
        version: opts.version.clone(),
        published_at: Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        assets: entries.clone(),
    };

    // Write checksums.txt.
    let checksums_content = format_checksums_txt(&entries);
    let checksums_path = opts.output_dir.join("checksums.txt");
    fs::write(&checksums_path, &checksums_content)?;

    // Write artifact-manifest.json.
    let manifest_path = opts.output_dir.join("artifact-manifest.json");
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    fs::write(&manifest_path, manifest_json)?;

    // Cosign placeholder — no OIDC locally, never fake a .sig.
    let placeholder_path = opts.output_dir.join("checksums.txt.sig-placeholder");
    fs::write(
        &placeholder_path,
        "# cosign keyless signature is generated by CI (GitHub Actions OIDC).\n\
         # Run `cosign verify-blob checksums.txt --certificate-identity-regexp '.*'\n\
         # --certificate-oidc-issuer https://token.actions.githubusercontent.com'\n\
         # against a real release to verify.\n",
    )?;

    Ok(ReleaseArtifacts {
        checksums_path,
        manifest_path,
        manifest,
        cosign_skipped: true,
    })
}

// ---------------------------------------------------------------------------
// Winget manifest generation
// ---------------------------------------------------------------------------

/// SHA-256 hashes needed to fill in the winget installer manifest.
pub struct WingetHashes {
    pub win_x64: String,
    pub win_arm64: String,
}

/// Build the three winget manifest YAML strings (version, installer, locale).
///
/// The manifests are assembled inline (explicit line join) — winget's nested
/// list mappings require real indentation that a `\`-continuation format! string
/// would strip. Returns (version_yaml, installer_yaml, locale_yaml).
pub fn generate_winget_manifests(version: &str, hashes: &WingetHashes) -> (String, String, String) {
    // `version` is the release tag (e.g. `v0.1.0`). winget-pkgs requires
    // PackageVersion (and the manifest directory name) to be the bare version
    // with no leading `v`; the download URL keeps the tag verbatim.
    let bare_version = version.strip_prefix('v').unwrap_or(version);
    let installer_url = |arch: &str| {
        format!(
            "https://github.com/monkey1wizard/golem-agents-legion/releases/download/{version}/gal-{version}-windows-{arch}.zip"
        )
    };

    // NOTE: lines are assembled explicitly (join on "\n") rather than via a `\`
    // line-continuation format! string — the continuation strips the source-side
    // leading whitespace, which for winget's nested list mappings produces
    // column-1 sub-keys and a fatal `[YAML:Parser] did not find expected key`.
    let version_yaml = [
        "PackageIdentifier: Monkey1Wizard.GAL".to_string(),
        format!("PackageVersion: \"{bare_version}\""),
        "DefaultLocale: en-US".to_string(),
        "ManifestType: version".to_string(),
        "ManifestVersion: 1.6.0".to_string(),
        String::new(),
    ]
    .join("\n");

    // Both the x64 and arm64 zips have the SAME flat root: a bare `gal.exe`
    // (release.yml stages `pkg/gal.exe` then archives `pkg/*`). So a single
    // top-level NestedInstallerFiles with RelativeFilePath `gal.exe` covers both
    // architectures — no per-installer, no versioned filename. List-item sub-keys
    // are indented 4 spaces to align under the first key after `- `.
    let installer_yaml = [
        "PackageIdentifier: Monkey1Wizard.GAL".to_string(),
        format!("PackageVersion: \"{bare_version}\""),
        "Commands:".to_string(),
        "  - gal".to_string(),
        "InstallerLocale: en-US".to_string(),
        "InstallerType: zip".to_string(),
        "NestedInstallerType: portable".to_string(),
        "NestedInstallerFiles:".to_string(),
        "  - RelativeFilePath: \"gal.exe\"".to_string(),
        "    PortableCommandAlias: gal".to_string(),
        "Installers:".to_string(),
        "  - Architecture: x64".to_string(),
        format!("    InstallerUrl: \"{}\"", installer_url("x64")),
        format!("    InstallerSha256: \"{}\"", hashes.win_x64),
        "  - Architecture: arm64".to_string(),
        format!("    InstallerUrl: \"{}\"", installer_url("arm64")),
        format!("    InstallerSha256: \"{}\"", hashes.win_arm64),
        "ManifestType: installer".to_string(),
        "ManifestVersion: 1.6.0".to_string(),
        String::new(),
    ]
    .join("\n");

    let locale_yaml = [
        "PackageIdentifier: Monkey1Wizard.GAL".to_string(),
        format!("PackageVersion: \"{bare_version}\""),
        "PackageLocale: en-US".to_string(),
        "Publisher: monkey1wizard".to_string(),
        "PublisherUrl: \"https://github.com/monkey1wizard\"".to_string(),
        "PublisherSupportUrl: \"https://github.com/monkey1wizard/golem-agents-legion/issues\""
            .to_string(),
        "PackageName: GAL".to_string(),
        "PackageUrl: \"https://github.com/monkey1wizard/golem-agents-legion\"".to_string(),
        "License: MIT".to_string(),
        "LicenseUrl: \"https://github.com/monkey1wizard/golem-agents-legion/blob/main/LICENSE\""
            .to_string(),
        "ShortDescription: \"GAL — Golem Agents Legion bootstrap CLI\"".to_string(),
        "Description: >-".to_string(),
        "  GAL (Golem Agents Legion) is a document-driven AI working system for solo".to_string(),
        "  developers. The `gal` binary provides init, update, doctor, and pipeline".to_string(),
        "  commands that drive the GAL document-driven workflow across".to_string(),
        "  Claude Code, GitHub Copilot, Codex, Antigravity CLI, and opencode.".to_string(),
        "Moniker: gal".to_string(),
        "Tags:".to_string(),
        "  - ai".to_string(),
        "  - cli".to_string(),
        "  - developer-tools".to_string(),
        "  - golem".to_string(),
        "  - golem-agents-legion".to_string(),
        "ManifestType: defaultLocale".to_string(),
        "ManifestVersion: 1.6.0".to_string(),
        String::new(),
    ]
    .join("\n");

    (version_yaml, installer_yaml, locale_yaml)
}

/// Write generated winget manifests to `output_dir/manifests/m/Monkey1Wizard/GAL/<version>/`.
///
/// Returns the paths to the three written files.
pub fn write_winget_manifests(
    output_dir: &Path,
    version: &str,
    hashes: &WingetHashes,
) -> Result<[PathBuf; 3], ReleaseError> {
    // winget-pkgs requires the manifest directory name == PackageVersion (bare,
    // no leading `v`). The InstallerUrl inside the manifest keeps the tag.
    let bare_version = version.strip_prefix('v').unwrap_or(version);
    let manifest_dir = output_dir
        .join("manifests")
        .join("m")
        .join("Monkey1Wizard")
        .join("GAL")
        .join(bare_version);
    fs::create_dir_all(&manifest_dir)?;

    let (version_yaml, installer_yaml, locale_yaml) = generate_winget_manifests(version, hashes);

    let version_path = manifest_dir.join("Monkey1Wizard.GAL.yaml");
    let installer_path = manifest_dir.join("Monkey1Wizard.GAL.installer.yaml");
    let locale_path = manifest_dir.join("Monkey1Wizard.GAL.locale.en-US.yaml");

    fs::write(&version_path, version_yaml)?;
    fs::write(&installer_path, installer_yaml)?;
    fs::write(&locale_path, locale_yaml)?;

    Ok([version_path, installer_path, locale_path])
}

// ---------------------------------------------------------------------------
// Homebrew formula generation
// ---------------------------------------------------------------------------

/// SHA-256 hashes for the Homebrew formula (macOS + Linux archives).
pub struct HomebrewHashes {
    pub darwin_x64: String,
    pub darwin_arm64: String,
    pub linux_x64: String,
    pub linux_arm64: String,
}

/// Generate the Homebrew formula content for a given version and hashes.
///
/// The formula installs the canonical archived binary
/// as `bin/gal` via `bin.install`. No `~/.gal/` seeding.
pub fn generate_homebrew_formula(version: &str, hashes: &HomebrewHashes) -> String {
    // `version` is the release tag (e.g. `v0.1.0`). The download URL/asset names
    // keep the tag verbatim. No explicit `version` stanza: Homebrew scans the
    // bare semver straight out of the `url` line, and `brew audit --strict`
    // flags a hand-written stanza as redundant once that scan succeeds.
    // `gal --version` prints the bare semver (`gal 0.1.0`), which is what
    // `assert_match version.to_s` compares against in the `test do` block.
    let dl = |plat: &str, arch: &str| {
        format!(
            "https://github.com/monkey1wizard/golem-agents-legion/releases/download/{version}/gal-{version}-{plat}-{arch}.tar.gz"
        )
    };
    // Lines are assembled explicitly (join on "\n") with real 2-space
    // indentation: a `\`-continuation format! string strips the source-side
    // leading whitespace, producing a flush-left formula that trips 15
    // `brew audit --strict` indentation offenses. `#{bin}` is a literal here
    // (Rust does not interpolate `#{}`); `pkgshare` == share/"gal".
    [
        "# Homebrew formula for GAL — Golem Agents Legion bootstrap CLI".to_string(),
        "# Generated by: gal release --homebrew".to_string(),
        "# Submit to: https://github.com/monkey1wizard/homebrew-tap".to_string(),
        "class Gal < Formula".to_string(),
        "  desc \"Document-driven AI working system for cross-tool developer workflows\""
            .to_string(),
        "  homepage \"https://github.com/monkey1wizard/golem-agents-legion\"".to_string(),
        "  license \"MIT\"".to_string(),
        String::new(),
        "  on_macos do".to_string(),
        "    if Hardware::CPU.arm?".to_string(),
        format!("      url \"{}\"", dl("darwin", "arm64")),
        format!("      sha256 \"{}\"", hashes.darwin_arm64),
        "    else".to_string(),
        format!("      url \"{}\"", dl("darwin", "x64")),
        format!("      sha256 \"{}\"", hashes.darwin_x64),
        "    end".to_string(),
        "  end".to_string(),
        String::new(),
        "  on_linux do".to_string(),
        "    if Hardware::CPU.arm?".to_string(),
        format!("      url \"{}\"", dl("linux", "arm64")),
        format!("      sha256 \"{}\"", hashes.linux_arm64),
        "    else".to_string(),
        format!("      url \"{}\"", dl("linux", "x64")),
        format!("      sha256 \"{}\"", hashes.linux_x64),
        "    end".to_string(),
        "  end".to_string(),
        String::new(),
        "  def install".to_string(),
        "    # Flat archive: bare `gal` binary + the whole gal-core source payload".to_string(),
        "    # side by side. Install the binary, then the rest of the tree into".to_string(),
        "    # pkgshare (share/gal) so `gal refresh` resolves an FHS source root.".to_string(),
        "    bin.install \"gal\"".to_string(),
        "    pkgshare.install Dir[\"*\"]".to_string(),
        "  end".to_string(),
        String::new(),
        "  test do".to_string(),
        "    assert_match version.to_s, shell_output(\"#{bin}/gal --version\")".to_string(),
        "    assert_predicate pkgshare/\"skills\", :directory?".to_string(),
        "    assert_predicate pkgshare/\"agents\", :directory?".to_string(),
        "  end".to_string(),
        "end".to_string(),
        String::new(),
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn dummy_asset(tmp: &TempDir, name: &str, content: &[u8]) -> AssetSpec {
        let path = tmp.path().join(name);
        fs::write(&path, content).unwrap();
        AssetSpec {
            name: name.to_string(),
            platform: "windows".to_string(),
            architecture: "x64".to_string(),
            kind: ArtifactKind::Binary,
            contains: vec![],
            path,
        }
    }

    // ─── SHA-256 + checksums ─────────────────────────────────────────

    #[test]
    fn sha256_file_produces_hex_digest() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.bin");
        fs::write(&path, b"hello gal").unwrap();
        let digest = sha256_file(&path).unwrap();
        assert_eq!(digest.len(), 64);
        assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn sha256_file_errors_on_missing() {
        let result = sha256_file(Path::new("/nonexistent/path/file.bin"));
        assert!(matches!(result, Err(ReleaseError::AssetNotFound(_))));
    }

    #[test]
    fn format_checksums_txt_sorted_lines() {
        let entries = vec![
            ArtifactEntry {
                name: "gal-v0.1.0-windows-x64.exe".to_string(),
                platform: "windows".to_string(),
                architecture: "x64".to_string(),
                sha256: "aaa".to_string(),
                kind: ArtifactKind::Binary,
                contains: vec![],
            },
            ArtifactEntry {
                name: "gal-v0.1.0-darwin-x64".to_string(),
                platform: "darwin".to_string(),
                architecture: "x64".to_string(),
                sha256: "bbb".to_string(),
                kind: ArtifactKind::Binary,
                contains: vec![],
            },
        ];
        let txt = format_checksums_txt(&entries);
        let lines: Vec<&str> = txt.lines().collect();
        // Sorted: darwin comes before windows.
        assert!(lines[0].contains("darwin"));
        assert!(lines[1].contains("windows"));
        // Format: "<sha256>  <name>"
        assert!(lines[0].starts_with("bbb  "));
        assert!(lines[1].starts_with("aaa  "));
    }

    #[test]
    fn canonical_binary_name_windows() {
        assert_eq!(
            canonical_binary_name("v0.1.0", "windows", "x64"),
            "gal-v0.1.0-windows-x64.exe"
        );
    }

    #[test]
    fn canonical_binary_name_unix() {
        assert_eq!(
            canonical_binary_name("v0.1.0", "linux", "arm64"),
            "gal-v0.1.0-linux-arm64"
        );
        assert_eq!(
            canonical_binary_name("v0.1.0", "darwin", "x64"),
            "gal-v0.1.0-darwin-x64"
        );
    }

    #[test]
    fn canonical_archive_names() {
        assert_eq!(
            canonical_archive_name("v0.1.0", "windows", "x64"),
            "gal-v0.1.0-windows-x64.zip"
        );
        assert_eq!(
            canonical_archive_name("v0.1.0", "darwin", "arm64"),
            "gal-v0.1.0-darwin-arm64.tar.gz"
        );
        assert_eq!(
            canonical_archive_name("v0.1.0", "linux", "x64"),
            "gal-v0.1.0-linux-x64.tar.gz"
        );
    }

    #[test]
    fn release_asset_matrix_has_12_entries() {
        // 6 platforms × 2 (binary + archive) = 12.
        let matrix = release_asset_matrix("v0.1.0");
        assert_eq!(matrix.len(), 12);
        let binaries: Vec<_> = matrix
            .iter()
            .filter(|e| e.3 == ArtifactKind::Binary)
            .collect();
        let archives: Vec<_> = matrix
            .iter()
            .filter(|e| e.3 == ArtifactKind::Archive)
            .collect();
        assert_eq!(binaries.len(), 6);
        assert_eq!(archives.len(), 6);
    }

    // ─── run_release (checksums + manifest combined) ─────────────────

    #[test]
    fn run_release_produces_required_files() {
        let tmp = TempDir::new().unwrap();
        let out_dir = tmp.path().join("release-out");

        let asset_name = canonical_binary_name("v0.1.0", "windows", "x64");
        let asset = dummy_asset(&tmp, &asset_name, b"fake binary content");

        let result = run_release(ReleaseOptions {
            version: "v0.1.0".to_string(),
            output_dir: out_dir.clone(),
            assets: vec![asset],
        })
        .unwrap();

        assert!(result.checksums_path.exists(), "checksums.txt must exist");
        assert!(
            result.manifest_path.exists(),
            "artifact-manifest.json must exist"
        );
        assert!(result.cosign_skipped, "cosign must be skipped locally");
        assert!(
            !out_dir.join("checksums.txt.sig").exists(),
            "must NOT produce a fake .sig"
        );
        assert!(
            out_dir.join("checksums.txt.sig-placeholder").exists(),
            "placeholder must exist"
        );
    }

    #[test]
    fn run_release_manifest_has_required_fields() {
        let tmp = TempDir::new().unwrap();
        let out_dir = tmp.path().join("release-out");

        let asset_name = canonical_binary_name("v0.1.0", "windows", "x64");
        let asset = dummy_asset(&tmp, &asset_name, b"content");

        let result = run_release(ReleaseOptions {
            version: "v0.1.0".to_string(),
            output_dir: out_dir.clone(),
            assets: vec![asset],
        })
        .unwrap();

        let manifest = &result.manifest;
        assert_eq!(manifest.version, "v0.1.0");
        assert!(!manifest.published_at.is_empty());
        assert_eq!(manifest.assets.len(), 1);

        let entry = &manifest.assets[0];
        assert_eq!(entry.name, asset_name);
        assert_eq!(entry.platform, "windows");
        assert_eq!(entry.architecture, "x64");
        assert_eq!(entry.kind, ArtifactKind::Binary);
        assert_eq!(entry.sha256.len(), 64);

        let json_path = out_dir.join("artifact-manifest.json");
        let json_content = fs::read_to_string(&json_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_content).unwrap();
        assert!(parsed["version"].is_string());
        assert!(parsed["publishedAt"].is_string());
        assert!(parsed["assets"].is_array());
        let asset_json = &parsed["assets"][0];
        assert!(asset_json["name"].is_string());
        assert!(asset_json["sha256"].is_string());
        assert!(asset_json["platform"].is_string());
        assert!(asset_json["architecture"].is_string());
        assert!(asset_json["kind"].is_string());
    }

    #[test]
    fn run_release_checksums_format() {
        let tmp = TempDir::new().unwrap();
        let out_dir = tmp.path().join("release-out");

        let asset_name = canonical_binary_name("v0.1.0", "linux", "x64");
        let asset = {
            let path = tmp.path().join(&asset_name);
            fs::write(&path, b"linux binary").unwrap();
            AssetSpec {
                name: asset_name.clone(),
                platform: "linux".to_string(),
                architecture: "x64".to_string(),
                kind: ArtifactKind::Binary,
                contains: vec![],
                path,
            }
        };

        run_release(ReleaseOptions {
            version: "v0.1.0".to_string(),
            output_dir: out_dir.clone(),
            assets: vec![asset],
        })
        .unwrap();

        let txt = fs::read_to_string(out_dir.join("checksums.txt")).unwrap();
        for line in txt.lines() {
            let parts: Vec<&str> = line.splitn(2, "  ").collect();
            assert_eq!(parts.len(), 2, "expected '  ' separator in: {line}");
            assert_eq!(parts[0].len(), 64, "SHA-256 must be 64 hex chars");
            assert!(parts[0].chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    // ─── winget manifest ─────────────────────────────────────────────

    #[test]
    fn winget_version_manifest_has_package_identifier() {
        let hashes = WingetHashes {
            win_x64: "a".repeat(64),
            win_arm64: "b".repeat(64),
        };
        let (version_yaml, _, _) = generate_winget_manifests("v0.1.0", &hashes);
        assert!(
            version_yaml.contains("PackageIdentifier: Monkey1Wizard.GAL"),
            "version manifest must contain PackageIdentifier: Monkey1Wizard.GAL\n{version_yaml}"
        );
        assert!(version_yaml.contains("ManifestType: version"));
        assert!(version_yaml.contains("ManifestVersion: 1.6.0"));
        // PackageVersion must be the bare version (no leading `v`).
        assert!(version_yaml.contains("PackageVersion: \"0.1.0\""));
        assert!(!version_yaml.contains("PackageVersion: \"v0.1.0\""));
    }

    #[test]
    fn winget_installer_manifest_url_points_to_release_artifact() {
        let version = "v0.1.0";
        let hashes = WingetHashes {
            win_x64: "a".repeat(64),
            win_arm64: "b".repeat(64),
        };
        let (_, installer_yaml, _) = generate_winget_manifests(version, &hashes);
        assert!(
            installer_yaml.contains(&format!("gal-{version}-windows-x64.zip")),
            "installer must reference x64 archive asset\n{installer_yaml}"
        );
        assert!(
            installer_yaml.contains(&format!("gal-{version}-windows-arm64.zip")),
            "installer must reference arm64 archive asset\n{installer_yaml}"
        );
        assert!(installer_yaml.contains("PackageIdentifier: Monkey1Wizard.GAL"));
        assert!(installer_yaml.contains("Architecture: x64"));
        assert!(installer_yaml.contains("Architecture: arm64"));
        assert!(installer_yaml.contains("ManifestType: installer"));
        // NestedInstallerFiles points at the archive-root bare gal.exe — no
        // versioned filename (the old `gal-<version>-windows-*.exe` assumption
        // never existed in the archive and blocked validation).
        assert!(installer_yaml.contains("RelativeFilePath: \"gal.exe\""));
        assert!(!installer_yaml.contains("windows-x64.exe"));
        assert!(!installer_yaml.contains("windows-arm64.exe"));
        // PackageVersion bare; InstallerUrl keeps the tag.
        assert!(installer_yaml.contains("PackageVersion: \"0.1.0\""));
        assert!(installer_yaml.contains("download/v0.1.0/"));
        // Nested list-item sub-keys MUST be indented 4 spaces (aligned under the
        // key after `- `), else winget rejects with a fatal YAML parse error.
        assert!(installer_yaml.contains("\n  - Architecture: x64"));
        assert!(installer_yaml.contains("\n    InstallerUrl: \"https"));
        assert!(installer_yaml.contains("\n    InstallerSha256: \""));
        assert!(installer_yaml.contains("\n    PortableCommandAlias: gal"));
    }

    #[test]
    fn winget_installer_sha256_matches_provided_hash() {
        let sha_x64 = "c".repeat(64);
        let sha_arm64 = "d".repeat(64);
        let hashes = WingetHashes {
            win_x64: sha_x64.clone(),
            win_arm64: sha_arm64.clone(),
        };
        let (_, installer_yaml, _) = generate_winget_manifests("v0.1.0", &hashes);
        assert!(
            installer_yaml.contains(&sha_x64),
            "installer must include x64 sha256\n{installer_yaml}"
        );
        assert!(
            installer_yaml.contains(&sha_arm64),
            "installer must include arm64 sha256\n{installer_yaml}"
        );
    }

    #[test]
    fn winget_locale_manifest_has_required_fields() {
        let hashes = WingetHashes {
            win_x64: "a".repeat(64),
            win_arm64: "b".repeat(64),
        };
        let (_, _, locale_yaml) = generate_winget_manifests("v0.1.0", &hashes);
        assert!(locale_yaml.contains("PackageIdentifier: Monkey1Wizard.GAL"));
        assert!(locale_yaml.contains("Publisher: monkey1wizard"));
        assert!(locale_yaml.contains("License: MIT"));
        assert!(locale_yaml.contains("Moniker: gal"));
        assert!(locale_yaml.contains("ManifestType: defaultLocale"));
    }

    #[test]
    fn write_winget_manifests_creates_directory_structure() {
        let tmp = TempDir::new().unwrap();
        let hashes = WingetHashes {
            win_x64: "a".repeat(64),
            win_arm64: "b".repeat(64),
        };
        let paths = write_winget_manifests(tmp.path(), "v0.1.0", &hashes).unwrap();
        assert!(paths[0].exists(), "version manifest must be written");
        assert!(paths[1].exists(), "installer manifest must be written");
        assert!(paths[2].exists(), "locale manifest must be written");
        // Manifest dir name == PackageVersion (bare, no `v`).
        let expected_dir = tmp.path().join("manifests/m/Monkey1Wizard/GAL/0.1.0");
        assert!(
            expected_dir.is_dir(),
            "manifest dir must be the bare version"
        );
        let versioned_dir = tmp.path().join("manifests/m/Monkey1Wizard/GAL/v0.1.0");
        assert!(
            !versioned_dir.exists(),
            "manifest dir must NOT carry a leading v"
        );
    }

    // ─── Homebrew formula ────────────────────────────────────────────

    #[test]
    fn homebrew_formula_has_required_structure() {
        let hashes = HomebrewHashes {
            darwin_x64: "e".repeat(64),
            darwin_arm64: "f".repeat(64),
            linux_x64: "g".repeat(64),
            linux_arm64: "h".repeat(64),
        };
        let formula = generate_homebrew_formula("v0.1.0", &hashes);
        assert!(formula.contains("class Gal < Formula"));
        // No explicit `version` stanza — Homebrew scans it from the `url` line,
        // and `brew audit --strict` rejects a hand-written one as redundant.
        assert!(!formula.contains("  version \""));
        // The download URL/asset names keep the tag verbatim.
        assert!(formula.contains("gal-v0.1.0-darwin-arm64.tar.gz"));
        assert!(formula.contains("gal-v0.1.0-darwin-x64.tar.gz"));
        assert!(formula.contains("gal-v0.1.0-linux-arm64.tar.gz"));
        assert!(formula.contains("gal-v0.1.0-linux-x64.tar.gz"));
        assert!(formula.contains("license \"MIT\""));
        // desc must NOT start with the formula name (brew audit) — no leading GAL/Gal.
        assert!(!formula.contains("desc \"GAL"));
        assert!(!formula.contains("desc \"Gal"));
        assert!(formula.contains("desc \"Document-driven"));
        // Layout contract: bare bin.install + whole payload into pkgshare, no glob.
        assert!(formula.contains("bin.install \"gal\""));
        assert!(formula.contains("pkgshare.install Dir[\"*\"]"));
        assert!(!formula.contains("Dir[\"gal-*\"]"));
        // pkgshare (not share/"gal") — brew audit --strict rejects the latter.
        assert!(!formula.contains("share/\"gal"));
        // test block asserts the payload landed under pkgshare.
        assert!(formula.contains("gal --version"));
        assert!(formula.contains("pkgshare/\"skills\""));
        assert!(formula.contains("pkgshare/\"agents\""));
        // Indentation must be real 2-space (brew audit --strict); a `\`-continuation
        // regression would flush everything to column 1.
        assert!(formula.contains("\n  desc \""));
        assert!(formula.contains("\n      url \"https"));
        assert!(formula.contains("\n    bin.install"));
    }

    #[test]
    fn homebrew_formula_sha256_values_embedded() {
        let hashes = HomebrewHashes {
            darwin_x64: "x64hash".to_string(),
            darwin_arm64: "arm64hash".to_string(),
            linux_x64: "linuxx64hash".to_string(),
            linux_arm64: "linuxarm64hash".to_string(),
        };
        let formula = generate_homebrew_formula("v0.1.0", &hashes);
        assert!(formula.contains("x64hash"));
        assert!(formula.contains("arm64hash"));
        assert!(formula.contains("linuxx64hash"));
        assert!(formula.contains("linuxarm64hash"));
    }
}
