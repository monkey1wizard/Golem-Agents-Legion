//! Release artifact generation for `gal release`.
//!
//! Produces `checksums.txt` and `artifact-manifest.json` as defined in
//! `docs/devguide.md §Release Artifact Matrix`. In local dry-run mode the
//! module uses the current executable as the only asset; in CI the workflow
//! feeds in the cross-compiled binaries and archives.
//!
//! Cosign keyless signing is a CI-only operation (needs OIDC token).
//! Locally, `run_release` writes a placeholder note instead of a fake `.sig`.
//!
//! Corresponds to T-014 of fix-gal-bootstrap-install-convergence.

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
    let ext = if platform == "windows" { ".zip" } else { ".tar.gz" };
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
        let exe = std::env::current_exe()
            .map_err(|e| ReleaseError::ExePath(e.to_string()))?;
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

#[cfg(test)]
mod tests {
    use super::*;
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

    // TP-020: dry-run produces checksums.txt + artifact-manifest.json per matrix spec.

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
        let binaries: Vec<_> = matrix.iter().filter(|e| e.3 == ArtifactKind::Binary).collect();
        let archives: Vec<_> = matrix.iter().filter(|e| e.3 == ArtifactKind::Archive).collect();
        assert_eq!(binaries.len(), 6);
        assert_eq!(archives.len(), 6);
    }

    #[test]
    fn sha256_file_produces_hex_digest() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.bin");
        fs::write(&path, b"hello gal").unwrap();
        let digest = sha256_file(&path).unwrap();
        // SHA-256 of "hello gal" — just verify format (64 hex chars).
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

        // TP-020: checksums.txt and artifact-manifest.json must exist.
        assert!(result.checksums_path.exists(), "checksums.txt must exist");
        assert!(result.manifest_path.exists(), "artifact-manifest.json must exist");

        // TP-021: cosign skipped locally — placeholder written, no fake .sig.
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
        assert_eq!(entry.sha256.len(), 64); // valid SHA-256 hex

        // TP-020: manifest JSON is parseable and has correct shape.
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
        // Each line: "<64-hex-chars>  <filename>"
        for line in txt.lines() {
            let parts: Vec<&str> = line.splitn(2, "  ").collect();
            assert_eq!(parts.len(), 2, "expected '  ' separator in: {line}");
            assert_eq!(parts[0].len(), 64, "SHA-256 must be 64 hex chars");
            assert!(parts[0].chars().all(|c| c.is_ascii_hexdigit()));
        }
    }
}
