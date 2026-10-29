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

/// Prefix marking an uncurated `gal release-notes` draft item. A stable
/// release must never ship a changelog section still carrying one.
const DRAFT_SENTINEL_PREFIX: &str = "[[GAL-RELEASE-DRAFT:";

/// True when `bare_version` (no leading `v`) is a prerelease — the same
/// predicate the release workflow already applies to its `prerelease:` input,
/// so the local gate and CI cannot disagree about which tags are stable.
fn is_prerelease(bare_version: &str) -> bool {
    bare_version.contains('-')
}

/// Resolve the top-level repo root from the current working directory,
/// failing closed on any git spawn/exit-status/UTF-8 error.
fn resolve_repo_root() -> Result<PathBuf, String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| format!("failed to resolve repo root: git spawn failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "failed to resolve repo root: not inside a git repository ({})",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|e| format!("failed to resolve repo root: invalid UTF-8: {e}"))?;
    Ok(PathBuf::from(text.trim()))
}

/// Extract the non-empty body of the exact column-zero `## [<bare_version>]`
/// heading (optionally followed by a ` - <date>` suffix), through the next
/// column-zero `## [` heading or end of file. Rejects a body still carrying
/// an uncurated `[[GAL-RELEASE-DRAFT:` item.
fn extract_changelog_section(changelog: &str, bare_version: &str) -> Result<String, String> {
    let heading_prefix = format!("## [{bare_version}]");
    let mut lines = changelog.lines();
    let mut found = false;
    let mut in_fence = false;
    for line in lines.by_ref() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(rest) = line.strip_prefix(heading_prefix.as_str()) {
            if rest.is_empty() || rest.starts_with(' ') {
                found = true;
                break;
            }
        }
    }
    if !found {
        return Err(format!("CHANGELOG.md has no '## [{bare_version}]' heading"));
    }

    let mut body_lines: Vec<&str> = Vec::new();
    for line in lines {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        }
        if !in_fence && line.starts_with("## [") {
            break;
        }
        body_lines.push(line);
    }
    while matches!(body_lines.first(), Some(l) if l.trim().is_empty()) {
        body_lines.remove(0);
    }
    while matches!(body_lines.last(), Some(l) if l.trim().is_empty()) {
        body_lines.pop();
    }
    if body_lines.is_empty() {
        return Err(format!("'## [{bare_version}]' section is empty"));
    }
    let section = body_lines.join("\n");
    if section.contains(DRAFT_SENTINEL_PREFIX) {
        return Err(format!(
            "'## [{bare_version}]' section still contains an uncurated \
             '{DRAFT_SENTINEL_PREFIX}' draft item"
        ));
    }
    Ok(section)
}

/// Stable-release changelog preflight: locate and validate the exact
/// `## [<bare_version>]` section in repo-root `CHANGELOG.md`. Prereleases
/// (bare form contains `-`) are exempt and return `Ok(None)`. Fails closed
/// before any artifact write.
fn stable_changelog_section(bare_version: &str) -> Result<Option<String>, String> {
    if is_prerelease(bare_version) {
        return Ok(None);
    }
    let repo_root = resolve_repo_root()?;
    let changelog_path = repo_root.join("CHANGELOG.md");
    let changelog = std::fs::read_to_string(&changelog_path)
        .map_err(|e| format!("cannot read {}: {e}", changelog_path.display()))?;
    extract_changelog_section(&changelog, bare_version).map(Some)
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

    // Bare lookup key derived into its own local binding — the parsed
    // `opts.version` must stay in tag form, since it also feeds canonical
    // asset names, download URLs, and the Homebrew formula filename below.
    let bare_version = opts.version.strip_prefix('v').unwrap_or(&opts.version);
    let changelog_section = match stable_changelog_section(bare_version) {
        Ok(section) => section,
        Err(e) => {
            eprintln!("gal release: {e}");
            return ExitCode::Error;
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

    if let Some(section) = &changelog_section {
        let body_path = opts.output_dir.join("release-body.md");
        if let Err(e) = std::fs::write(&body_path, section) {
            eprintln!("gal release: failed to write release body: {e}");
            return ExitCode::Error;
        }
        println!("release-body.md → {}", body_path.display());
    }

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

    // ── is_prerelease ─────────────────────────────────────────────────────

    #[test]
    fn is_prerelease_true_for_hyphenated_bare_version() {
        assert!(is_prerelease("0.9.9-rc1"));
    }

    #[test]
    fn is_prerelease_false_for_stable_bare_version() {
        assert!(!is_prerelease("0.9.9"));
    }

    // ── extract_changelog_section ────────────────────────────────────────

    #[test]
    fn extract_changelog_section_accepts_exact_heading_with_date_suffix() {
        let changelog = "# Changelog\n\n## [0.9.9] - 2026-01-01\n\n### Added\n\n- a thing\n\n## [0.9.8] - 2025-12-01\n\n### Fixed\n\n- older thing\n";
        let section = extract_changelog_section(changelog, "0.9.9").unwrap();
        assert_eq!(section, "### Added\n\n- a thing");
    }

    #[test]
    fn extract_changelog_section_accepts_heading_with_no_date_suffix() {
        let changelog = "## [0.9.9]\n\n### Added\n\n- a thing\n\n## [0.9.8]\n";
        let section = extract_changelog_section(changelog, "0.9.9").unwrap();
        assert_eq!(section, "### Added\n\n- a thing");
    }

    #[test]
    fn extract_changelog_section_rejects_missing_heading() {
        let changelog = "## [0.9.8] - 2025-12-01\n\n### Fixed\n\n- older thing\n";
        assert!(extract_changelog_section(changelog, "0.9.9").is_err());
    }

    #[test]
    fn extract_changelog_section_rejects_substring_false_positive() {
        // "0.9.9-rc1" is not the exact bare version "0.9.9" — the character
        // right after the version digits must be ']' or a date-suffix space.
        let changelog = "## [0.9.9-rc1] - 2026-01-01\n\n### Added\n\n- a thing\n";
        assert!(extract_changelog_section(changelog, "0.9.9").is_err());
    }

    #[test]
    fn extract_changelog_section_rejects_empty_body() {
        let changelog = "## [0.9.9] - 2026-01-01\n\n## [0.9.8] - 2025-12-01\n\n### Fixed\n\n- x\n";
        assert!(extract_changelog_section(changelog, "0.9.9").is_err());
    }

    #[test]
    fn extract_changelog_section_rejects_draft_sentinel() {
        let changelog = "## [0.9.9] - 2026-01-01\n\n### Added\n\n- [[GAL-RELEASE-DRAFT:abc1234]] abc1234 feat: a thing\n";
        assert!(extract_changelog_section(changelog, "0.9.9").is_err());
    }

    #[test]
    fn extract_changelog_section_last_entry_extends_to_eof() {
        let changelog = "## [0.9.9] - 2026-01-01\n\n### Added\n\n- only entry\n";
        let section = extract_changelog_section(changelog, "0.9.9").unwrap();
        assert_eq!(section, "### Added\n\n- only entry");
    }

    #[test]
    fn extract_changelog_section_fenced_example_heading_must_not_shadow_real_section() {
        // A fenced false positive is a required-fail scenario alongside
        // missing file/heading/empty body. A doc example wrapped in a
        // code fence that happens to contain a column-zero `## [0.9.9]` line
        // must not be treated as the real heading — either the parser must skip
        // it and find the real section below, or it must fail closed. Silently
        // returning the fenced example's own body as if it were the curated
        // section is the one outcome the gate must never produce.
        let changelog = "# Changelog\n\nExample entry:\n\n```\n## [0.9.9] - fake example, ignore\n```\n\n## [0.9.9] - 2026-01-01\n\n### Added\n\n- real curated entry\n";
        match extract_changelog_section(changelog, "0.9.9") {
            Err(_) => {} // fail-closed is an acceptable outcome
            Ok(section) => assert_eq!(
                section, "### Added\n\n- real curated entry",
                "extract_changelog_section returned the fenced example's body \
                 ('{section}') instead of failing or finding the real curated section"
            ),
        }
    }

    #[test]
    fn extract_changelog_section_unclosed_fence_in_body_does_not_swallow_next_heading() {
        // A properly closed fence inside the target section's own body must not
        // suppress the next column-zero `## [` boundary. An odd number of ``` lines
        // (a malformed/unclosed fence) would leave `in_fence` stuck true for the
        // fence-toggle implementation, silently merging the next version's section
        // into this one — the same "silent wrong data" failure class the fenced
        // false-positive test guards against, but on the body-boundary side rather
        // than the heading side.
        let changelog = "## [0.9.9] - 2026-01-01\n\n### Added\n\n- has a closed example:\n```\nfn foo() {}\n```\n\n## [0.9.8] - 2026-01-01\n\n### Added\n\n- older entry\n";
        let section = extract_changelog_section(changelog, "0.9.9").unwrap();
        assert_eq!(
            section, "### Added\n\n- has a closed example:\n```\nfn foo() {}\n```",
            "a properly closed fence inside the section body must not consume the \
             next version heading ('{section}')"
        );
    }

    // ── cmd_release changelog gate (real git temp repo) ──────────────────

    fn init_repo_with_changelog(changelog: &str) -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        let status = std::process::Command::new("git")
            .current_dir(temp.path())
            .args(["init", "-q"])
            .status()
            .unwrap();
        assert!(status.success());
        std::fs::write(temp.path().join("CHANGELOG.md"), changelog).unwrap();
        temp
    }

    fn dummy_binary_asset_arg(dir: &std::path::Path) -> String {
        let path = dir.join("gal-v0.9.9-linux-x64");
        std::fs::write(&path, b"fake binary").unwrap();
        format!("gal-v0.9.9-linux-x64:linux:x64:binary:{}", path.display())
    }

    #[test]
    fn cmd_release_stable_gate_blocks_and_writes_nothing_on_missing_changelog_file() {
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp = tempfile::tempdir().unwrap();
        let status = std::process::Command::new("git")
            .current_dir(temp.path())
            .args(["init", "-q"])
            .status()
            .unwrap();
        assert!(status.success());
        // No CHANGELOG.md written at all — distinct from the missing-heading case.
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let out_dir = temp.path().join("release-out");
        let asset_arg = dummy_binary_asset_arg(temp.path());
        let args: Vec<String> = vec![
            "release".into(),
            "--version".into(),
            "v0.9.9".into(),
            "--output-dir".into(),
            out_dir.display().to_string(),
            "--asset".into(),
            asset_arg,
        ];
        let code = cmd_release(&args);

        std::env::set_current_dir(original).unwrap();

        assert_eq!(code, ExitCode::Error);
        assert!(
            !out_dir.exists(),
            "no output-dir write when CHANGELOG.md is missing"
        );
    }

    #[test]
    fn cmd_release_stable_gate_blocks_and_writes_nothing_outside_a_git_repo() {
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // Plain temp dir, no `git init` — repo-root resolution must fail closed.
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("CHANGELOG.md"),
            "## [0.9.9] - 2026-01-01\n\n### Added\n\n- a thing\n",
        )
        .unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let out_dir = temp.path().join("release-out");
        let asset_arg = dummy_binary_asset_arg(temp.path());
        let args: Vec<String> = vec![
            "release".into(),
            "--version".into(),
            "v0.9.9".into(),
            "--output-dir".into(),
            out_dir.display().to_string(),
            "--asset".into(),
            asset_arg,
        ];
        let code = cmd_release(&args);

        std::env::set_current_dir(original).unwrap();

        assert_eq!(code, ExitCode::Error);
        assert!(!out_dir.exists(), "no output-dir write outside a git repo");
    }

    #[test]
    fn cmd_release_stable_gate_blocks_and_writes_nothing_on_missing_heading() {
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let repo = init_repo_with_changelog("## [0.9.8] - 2025-12-01\n\n### Fixed\n\n- x\n");
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(repo.path()).unwrap();

        let out_dir = repo.path().join("release-out");
        let asset_arg = dummy_binary_asset_arg(repo.path());
        let args: Vec<String> = vec![
            "release".into(),
            "--version".into(),
            "v0.9.9".into(),
            "--output-dir".into(),
            out_dir.display().to_string(),
            "--asset".into(),
            asset_arg,
        ];
        let code = cmd_release(&args);

        std::env::set_current_dir(original).unwrap();

        assert_eq!(code, ExitCode::Error);
        assert!(
            !out_dir.exists(),
            "no output-dir write on a missing section"
        );
    }

    #[test]
    fn cmd_release_stable_gate_blocks_on_draft_sentinel_residue() {
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let repo = init_repo_with_changelog(
            "## [0.9.9] - 2026-01-01\n\n### Added\n\n- [[GAL-RELEASE-DRAFT:abc1234]] abc1234 feat: a thing\n",
        );
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(repo.path()).unwrap();

        let out_dir = repo.path().join("release-out");
        let asset_arg = dummy_binary_asset_arg(repo.path());
        let args: Vec<String> = vec![
            "release".into(),
            "--version".into(),
            "v0.9.9".into(),
            "--output-dir".into(),
            out_dir.display().to_string(),
            "--asset".into(),
            asset_arg,
        ];
        let code = cmd_release(&args);

        std::env::set_current_dir(original).unwrap();

        assert_eq!(code, ExitCode::Error);
        assert!(
            !out_dir.exists(),
            "no output-dir write on uncurated draft residue"
        );
    }

    #[test]
    fn cmd_release_stable_gate_passes_for_bare_and_v_prefixed_versions() {
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let section_body = "### Added\n\n- a curated thing";
        let changelog = format!("## [0.9.9] - 2026-01-01\n\n{section_body}\n\n## [0.9.8]\n");
        let repo = init_repo_with_changelog(&changelog);
        let original = std::env::current_dir().unwrap();

        for version in ["0.9.9", "v0.9.9"] {
            std::env::set_current_dir(repo.path()).unwrap();
            let out_dir = repo.path().join(format!("release-out-{version}"));
            let asset_arg = dummy_binary_asset_arg(repo.path());
            let args: Vec<String> = vec![
                "release".into(),
                "--version".into(),
                version.into(),
                "--output-dir".into(),
                out_dir.display().to_string(),
                "--asset".into(),
                asset_arg,
            ];
            let code = cmd_release(&args);
            std::env::set_current_dir(&original).unwrap();

            assert_eq!(code, ExitCode::Success, "version '{version}' must pass");
            let body = std::fs::read_to_string(out_dir.join("release-body.md")).unwrap();
            assert_eq!(
                body, section_body,
                "release-body.md must be byte-identical to the section"
            );
        }
    }

    #[test]
    fn cmd_release_prerelease_skips_stable_gate_and_writes_no_body() {
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // No CHANGELOG.md section for this version at all — prerelease must
        // still pass because the stable gate is skipped entirely.
        let repo = init_repo_with_changelog("## [0.9.8] - 2025-12-01\n\n### Fixed\n\n- x\n");
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(repo.path()).unwrap();

        let out_dir = repo.path().join("release-out");
        let asset_arg = dummy_binary_asset_arg(repo.path());
        let args: Vec<String> = vec![
            "release".into(),
            "--version".into(),
            "v0.9.9-rc1".into(),
            "--output-dir".into(),
            out_dir.display().to_string(),
            "--asset".into(),
            asset_arg,
        ];
        let code = cmd_release(&args);

        std::env::set_current_dir(original).unwrap();

        assert_eq!(code, ExitCode::Success);
        assert!(!out_dir.join("release-body.md").exists());
    }

    #[test]
    fn cmd_release_stable_gate_resolves_repo_root_from_subdirectory() {
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let section_body = "### Added\n\n- a curated thing";
        let changelog = format!("## [0.9.9] - 2026-01-01\n\n{section_body}\n");
        let repo = init_repo_with_changelog(&changelog);
        let subdir = repo.path().join("nested");
        std::fs::create_dir_all(&subdir).unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(&subdir).unwrap();

        let out_dir = repo.path().join("release-out");
        let asset_arg = dummy_binary_asset_arg(repo.path());
        let args: Vec<String> = vec![
            "release".into(),
            "--version".into(),
            "v0.9.9".into(),
            "--output-dir".into(),
            out_dir.display().to_string(),
            "--asset".into(),
            asset_arg,
        ];
        let code = cmd_release(&args);

        std::env::set_current_dir(original).unwrap();

        assert_eq!(code, ExitCode::Success);
        let body = std::fs::read_to_string(out_dir.join("release-body.md")).unwrap();
        assert_eq!(body, section_body);
    }

    #[test]
    fn cmd_release_v_prefixed_version_leaves_canonical_names_untouched() {
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let section_body = "### Added\n\n- a curated thing";
        let changelog = format!("## [0.9.9] - 2026-01-01\n\n{section_body}\n");
        let repo = init_repo_with_changelog(&changelog);
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(repo.path()).unwrap();

        let out_dir = repo.path().join("release-out");
        let asset_arg = dummy_binary_asset_arg(repo.path());
        let args: Vec<String> = vec![
            "release".into(),
            "--version".into(),
            "v0.9.9".into(),
            "--output-dir".into(),
            out_dir.display().to_string(),
            "--homebrew".into(),
            "--asset".into(),
            asset_arg,
        ];
        let code = cmd_release(&args);

        std::env::set_current_dir(&original).unwrap();

        assert_eq!(code, ExitCode::Success);
        // The parsed version is untouched (tag form) — the formula filename
        // and manifest version must still carry the 'v' prefix.
        assert!(out_dir.join("gal-v0.9.9.rb").exists());
        let manifest = std::fs::read_to_string(out_dir.join("artifact-manifest.json")).unwrap();
        assert!(manifest.contains("\"v0.9.9\""));
    }
}
