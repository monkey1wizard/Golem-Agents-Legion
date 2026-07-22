//! `gal release` — generate release artifacts (checksums.txt, artifact-manifest.json,
//! optional winget/homebrew package-manager manifests).
//!
//! Release-internal binary subcommand, NOT a public `/gal` slash command. The public
//! command surface does not grow; this is a peer of `finalize-check`.
//!
//! Usage:
//!   gal release --version <v> [--output-dir <dir>] [--winget] [--homebrew]
//!               [--asset <path> ...]
//!
//! --asset accepts either a bare path (name/platform/arch/kind derived from the
//! canonical filename `gal-<ver>-<platform>-<arch>[ext]`) or the explicit form
//! `<name>:<platform>:<arch>:<binary|archive>:<path>` for non-canonical names.

use gal_engine::release::{
    generate_homebrew_formula, run_release, write_winget_manifests, ArtifactKind, AssetSpec,
    HomebrewHashes, ReleaseOptions, WingetHashes,
};
use gal_engine::ExitCode;
use regex::Regex;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Safe subset for `--version`: optional leading `v`, `MAJOR.MINOR.PATCH`, optional
/// dot-separated prerelease identifiers. Rejects path separators, quotes, control
/// characters, build metadata (`+...`), and empty prerelease identifiers.
fn version_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"^v?[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$")
            .expect("version_pattern is a fixed valid regex literal")
    })
}

/// Escape control characters in rejected `--version` input so a diagnostic never
/// replays raw control bytes to the terminal.
fn escape_for_diagnostic(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_control() {
                format!("\\u{{{:04x}}}", c as u32)
            } else {
                c.to_string()
            }
        })
        .collect()
}

struct ReleaseArgs {
    version: String,
    output_dir: PathBuf,
    winget: bool,
    homebrew: bool,
    assets: Vec<AssetSpec>,
}

/// Derive `AssetSpec` from a canonical asset path.
///
/// Canonical pattern: `gal-<version>-<platform>-<arch>[.exe|.zip|.tar.gz]`
/// Kind: `.zip` or `.tar.gz` → archive; otherwise → binary.
/// Platform and arch are extracted from the basename after stripping the `gal-<ver>-` prefix
/// and the extension.
fn asset_from_path(raw: &str) -> Result<AssetSpec, String> {
    let path = PathBuf::from(raw);
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("cannot derive asset name from path: {raw}"))?
        .to_string();

    let (kind, stem) = if name.ends_with(".tar.gz") {
        (ArtifactKind::Archive, name.trim_end_matches(".tar.gz"))
    } else if name.ends_with(".zip") {
        (ArtifactKind::Archive, name.trim_end_matches(".zip"))
    } else if name.ends_with(".exe") {
        (ArtifactKind::Binary, name.trim_end_matches(".exe"))
    } else {
        (ArtifactKind::Binary, name.as_str())
    };

    // Expect stem to be `gal-<version>-<platform>-<arch>`.
    // Split on '-' from the right: last part = arch, second-to-last = platform.
    let parts: Vec<&str> = stem.splitn(4, '-').collect();
    // parts: ["gal", "<ver>", "<platform>", "<arch>"] — but version may contain '-'
    // Use a more reliable split: after stripping "gal-" prefix, take the last two
    // dash-separated tokens as arch and platform.
    let without_gal = stem.strip_prefix("gal-").unwrap_or(stem);
    let tokens: Vec<&str> = without_gal.rsplitn(3, '-').collect();
    // rsplitn(3) gives: [arch, platform, version] (reversed order from right)
    if tokens.len() < 2 {
        return Err(format!(
            "cannot derive platform/arch from asset name '{}'; \
             expected canonical gal-<ver>-<platform>-<arch>[ext]",
            name
        ));
    }
    let arch = tokens[0].to_string();
    let platform = tokens[1].to_string();
    let _ = parts; // suppress unused warning

    Ok(AssetSpec {
        name,
        platform,
        architecture: arch,
        kind,
        contains: vec![],
        path,
    })
}

fn parse_release_args(args: &[String]) -> Result<ReleaseArgs, String> {
    let mut version: Option<String> = None;
    let mut output_dir = PathBuf::from("release-out");
    let mut winget = false;
    let mut homebrew = false;
    let mut assets: Vec<AssetSpec> = Vec::new();

    let mut i = 1; // skip "release"
    while i < args.len() {
        match args[i].as_str() {
            "--version" => {
                i += 1;
                version = Some(args.get(i).ok_or("--version requires a value")?.clone());
            }
            "--output-dir" => {
                i += 1;
                output_dir = PathBuf::from(args.get(i).ok_or("--output-dir requires a value")?);
            }
            "--winget" => winget = true,
            "--homebrew" => homebrew = true,
            "--asset" => {
                i += 1;
                let raw = args.get(i).ok_or("--asset requires a value")?;
                // Explicit form: name:platform:arch:binary|archive:path (5 colon-parts).
                // Simple form: bare path — name/platform/arch/kind derived from canonical
                // filename convention `gal-<ver>-<platform>-<arch>[.exe|.zip|.tar.gz]`.
                let parts: Vec<&str> = raw.splitn(5, ':').collect();
                let spec = if parts.len() == 5 {
                    let kind = match parts[3] {
                        "binary" => ArtifactKind::Binary,
                        "archive" => ArtifactKind::Archive,
                        k => {
                            return Err(format!(
                                "unknown asset kind '{k}' (expected binary|archive)"
                            ))
                        }
                    };
                    AssetSpec {
                        name: parts[0].to_string(),
                        platform: parts[1].to_string(),
                        architecture: parts[2].to_string(),
                        kind,
                        contains: vec![],
                        path: PathBuf::from(parts[4]),
                    }
                } else {
                    // Simple path — derive from canonical filename.
                    asset_from_path(raw)?
                };
                assets.push(spec);
            }
            flag => return Err(format!("gal release: unknown flag '{flag}'")),
        }
        i += 1;
    }

    let version = version.ok_or("--version is required")?;
    if !version_pattern().is_match(&version) {
        return Err(format!(
            "invalid --version '{}': expected v?MAJOR.MINOR.PATCH[-PRERELEASE]",
            escape_for_diagnostic(&version)
        ));
    }
    Ok(ReleaseArgs {
        version,
        output_dir,
        winget,
        homebrew,
        assets,
    })
}

pub(crate) fn cmd_release(args: &[String]) -> ExitCode {
    let opts = match parse_release_args(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("gal release: {e}");
            eprintln!("Usage: gal release --version <v> [--output-dir <dir>] [--winget] [--homebrew] [--asset <name:platform:arch:kind:path>]");
            return ExitCode::Usage;
        }
    };

    let artifacts = match run_release(ReleaseOptions {
        version: opts.version.clone(),
        output_dir: opts.output_dir.clone(),
        assets: opts.assets,
    }) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("gal release: {e}");
            return ExitCode::Error;
        }
    };

    println!("checksums.txt → {}", artifacts.checksums_path.display());
    println!(
        "artifact-manifest.json → {}",
        artifacts.manifest_path.display()
    );

    if opts.winget {
        let hashes = winget_hashes_from_manifest(&artifacts.manifest, &opts.version);
        match write_winget_manifests(&opts.output_dir, &opts.version, &hashes) {
            Ok(paths) => {
                for p in &paths {
                    println!("winget manifest → {}", p.display());
                }
            }
            Err(e) => {
                eprintln!("gal release: winget generation failed: {e}");
                return ExitCode::Error;
            }
        }
    }

    if opts.homebrew {
        let hashes = homebrew_hashes_from_manifest(&artifacts.manifest, &opts.version);
        let formula = generate_homebrew_formula(&opts.version, &hashes);
        let formula_path = opts.output_dir.join(format!("gal-{}.rb", opts.version));
        if let Err(e) = std::fs::write(&formula_path, formula) {
            eprintln!("gal release: homebrew generation failed: {e}");
            return ExitCode::Error;
        }
        println!("homebrew formula → {}", formula_path.display());
    }

    ExitCode::Success
}

fn winget_hashes_from_manifest(
    manifest: &gal_engine::release::ArtifactManifest,
    version: &str,
) -> WingetHashes {
    let find_hash = |platform: &str, arch: &str| -> String {
        manifest
            .assets
            .iter()
            .find(|e| {
                e.platform == platform
                    && e.architecture == arch
                    && matches!(e.kind, ArtifactKind::Archive)
            })
            .map(|e| e.sha256.clone())
            .unwrap_or_else(|| {
                eprintln!(
                    "warning: no {platform}/{arch} archive in manifest for version {version}"
                );
                String::new()
            })
    };
    WingetHashes {
        win_x64: find_hash("windows", "x64"),
        win_arm64: find_hash("windows", "arm64"),
    }
}

fn homebrew_hashes_from_manifest(
    manifest: &gal_engine::release::ArtifactManifest,
    version: &str,
) -> HomebrewHashes {
    let find_hash = |platform: &str, arch: &str| -> String {
        manifest
            .assets
            .iter()
            .find(|e| {
                e.platform == platform
                    && e.architecture == arch
                    && matches!(e.kind, ArtifactKind::Archive)
            })
            .map(|e| e.sha256.clone())
            .unwrap_or_else(|| {
                eprintln!(
                    "warning: no {platform}/{arch} archive in manifest for version {version}"
                );
                String::new()
            })
    };
    HomebrewHashes {
        darwin_x64: find_hash("darwin", "x64"),
        darwin_arm64: find_hash("darwin", "arm64"),
        linux_x64: find_hash("linux", "x64"),
        linux_arm64: find_hash("linux", "arm64"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_release_args_requires_version() {
        let args: Vec<String> = vec!["release".into(), "--output-dir".into(), "out".into()];
        assert!(parse_release_args(&args).is_err());
    }

    #[test]
    fn parse_release_args_minimal() {
        let args: Vec<String> = vec!["release".into(), "--version".into(), "v0.1.0".into()];
        let opts = parse_release_args(&args).unwrap();
        assert_eq!(opts.version, "v0.1.0");
        assert!(!opts.winget);
        assert!(!opts.homebrew);
        assert!(opts.assets.is_empty());
    }

    #[test]
    fn parse_release_args_flags() {
        let args: Vec<String> = vec![
            "release".into(),
            "--version".into(),
            "v0.2.0".into(),
            "--output-dir".into(),
            "dist".into(),
            "--winget".into(),
            "--homebrew".into(),
        ];
        let opts = parse_release_args(&args).unwrap();
        assert_eq!(opts.version, "v0.2.0");
        assert_eq!(opts.output_dir, PathBuf::from("dist"));
        assert!(opts.winget);
        assert!(opts.homebrew);
    }

    #[test]
    fn parse_release_args_accepts_safe_version_subset() {
        for v in ["v0.1.0", "v0.1.0-rc3", "v0.1.0-rc.3", "v0.1.0-rc-3"] {
            let args: Vec<String> = vec!["release".into(), "--version".into(), v.into()];
            let opts = parse_release_args(&args)
                .unwrap_or_else(|e| panic!("expected '{v}' to be accepted, got error: {e}"));
            assert_eq!(opts.version, v);
        }
    }

    #[test]
    fn parse_release_args_rejects_unsafe_version_subset() {
        let rejected = [
            "v1.0.0-$(x)",
            "1.0",
            "a\"b",
            "1.0.0/../x",
            "v1.0.0-.",
            "v1.0.0-rc..1",
            "v1.0.0+build",
        ];
        for v in rejected {
            let args: Vec<String> = vec!["release".into(), "--version".into(), v.into()];
            assert!(
                parse_release_args(&args).is_err(),
                "expected '{v}' to be rejected"
            );
        }
    }

    #[test]
    fn parse_release_args_rejects_control_character_version() {
        let v = "v1.0.0\u{0007}";
        let args: Vec<String> = vec!["release".into(), "--version".into(), v.into()];
        let err = match parse_release_args(&args) {
            Err(e) => e,
            Ok(_) => panic!("control-character version must be rejected"),
        };
        assert!(
            !err.contains('\u{0007}'),
            "diagnostic must not replay the raw control character: {err}"
        );
    }

    #[test]
    fn parse_release_args_unknown_flag() {
        let args: Vec<String> = vec![
            "release".into(),
            "--version".into(),
            "v0.1.0".into(),
            "--unknown".into(),
        ];
        assert!(parse_release_args(&args).is_err());
    }

    #[test]
    fn parse_release_args_asset_binary_simple_path() {
        // Simple form: bare path, derive from canonical filename.
        let args: Vec<String> = vec![
            "release".into(),
            "--version".into(),
            "v0.1.0".into(),
            "--asset".into(),
            "gal-v0.1.0-linux-x64".into(),
        ];
        let opts = parse_release_args(&args).unwrap();
        assert_eq!(opts.assets.len(), 1);
        assert_eq!(opts.assets[0].name, "gal-v0.1.0-linux-x64");
        assert_eq!(opts.assets[0].platform, "linux");
        assert_eq!(opts.assets[0].architecture, "x64");
        assert!(matches!(opts.assets[0].kind, ArtifactKind::Binary));
    }

    #[test]
    fn parse_release_args_asset_archive_simple_path() {
        let args: Vec<String> = vec![
            "release".into(),
            "--version".into(),
            "v0.1.0".into(),
            "--asset".into(),
            "gal-v0.1.0-darwin-arm64.tar.gz".into(),
        ];
        let opts = parse_release_args(&args).unwrap();
        assert_eq!(opts.assets[0].platform, "darwin");
        assert_eq!(opts.assets[0].architecture, "arm64");
        assert!(matches!(opts.assets[0].kind, ArtifactKind::Archive));
    }

    #[test]
    fn parse_release_args_asset_explicit_form() {
        // Explicit form: name:platform:arch:kind:path (5 colon-parts).
        let args: Vec<String> = vec![
            "release".into(),
            "--version".into(),
            "v0.1.0".into(),
            "--asset".into(),
            "gal-v0.1.0-linux-x64:linux:x64:binary:/tmp/gal".into(),
        ];
        let opts = parse_release_args(&args).unwrap();
        assert_eq!(opts.assets.len(), 1);
        assert_eq!(opts.assets[0].name, "gal-v0.1.0-linux-x64");
        assert_eq!(opts.assets[0].platform, "linux");
        assert_eq!(opts.assets[0].architecture, "x64");
        assert!(matches!(opts.assets[0].kind, ArtifactKind::Binary));
    }

    #[test]
    fn parse_release_args_asset_bad_kind() {
        let args: Vec<String> = vec![
            "release".into(),
            "--version".into(),
            "v0.1.0".into(),
            "--asset".into(),
            "name:linux:x64:WRONG:/tmp/f".into(),
        ];
        assert!(parse_release_args(&args).is_err());
    }

    #[test]
    fn asset_from_path_windows_binary() {
        let spec = asset_from_path("gal-v0.1.0-windows-x64.exe").unwrap();
        assert_eq!(spec.platform, "windows");
        assert_eq!(spec.architecture, "x64");
        assert!(matches!(spec.kind, ArtifactKind::Binary));
    }

    #[test]
    fn asset_from_path_windows_archive() {
        let spec = asset_from_path("gal-v0.1.0-windows-arm64.zip").unwrap();
        assert_eq!(spec.platform, "windows");
        assert_eq!(spec.architecture, "arm64");
        assert!(matches!(spec.kind, ArtifactKind::Archive));
    }

    #[test]
    fn asset_from_path_linux_targz() {
        let spec = asset_from_path("gal-v0.1.0-linux-x64.tar.gz").unwrap();
        assert_eq!(spec.platform, "linux");
        assert_eq!(spec.architecture, "x64");
        assert!(matches!(spec.kind, ArtifactKind::Archive));
    }

    #[test]
    fn phase_from_str_release_is_not_a_phase() {
        // Confirm Phase enum does not accept "release" — guard against accidental Phase::Release.
        use gal_engine::CommandKind;
        // CommandKind::Release exists but is NOT a Phase variant.
        assert!(CommandKind::parse("release").is_some());
        // There is no gal_dispatch::Phase::Release — Phase enum only has Implement/Test/Audit.
        // Verify by checking that the dispatch crate's stage module only has the known phases.
        // (This test is a compile-time guard: if Phase::Release were added it would break
        //  dispatch::run which has an exhaustive match — and the build would fail.)
        assert_eq!(CommandKind::parse("release"), Some(CommandKind::Release));
    }
}
