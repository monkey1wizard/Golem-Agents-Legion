//! `gal` CLI entry point — Rust-native install/update/doctor/uninstall/commit-msg.
//!
//! Entry switch (T-011, BUG-B): the Rust binary is the single entry for
//! install, update, uninstall, doctor, and commit-msg.
//!
//! T-011: install/update/uninstall wired to Rust-native gal_engine::install.
//! T-012: doctor wired to Rust-native gal_engine::doctor.
//! commit-msg: Rust-native generator + non-destructive hook in
//! gal_engine::commit_msg. This fully replaces the retired
//! Get-StagedCommitMessage.ps1 / get-staged-commit-message.sh helpers.

mod init_repo;

use gal_engine::{classify_args, Action, CommandKind, ExitCode};
use base::health::HealthCheck;
use std::path::{Path, PathBuf};
use std::process::ExitCode as ProcessExitCode;

fn print_help() {
    println!("gal — GAL bootstrap CLI");
    println!();
    println!("Usage: gal <command> [options]");
    println!("       gal --version");
    println!("       gal --help");
    println!();
    println!("Commands:");
    for cmd in CommandKind::ALL {
        println!("  {}", cmd.as_str());
    }
    println!();
    println!("Options:");
    println!("  doctor --dry-run                  Read-only health check (no filesystem changes)");
    println!("  sync [repo-root]                  Regenerate repo-local adapter files");
    println!("  setup [--check|--dry-run|--reconfigure|--uninstall [--purge --confirm-purge]]");
    println!("        [--replace] [--bootstrap-install] [--selected-runtimes <csv>] [--primary-runtime <v>]");
    println!("  setup --tools [--check] [--tool <gstack|graphify|opencli|xmachine>[,..]]");
    println!("  init-repo [targetPath] [projectName] [--blank] [--force]");
    println!("  resolve-catalog [--catalog-path <path>] [--config-path <path>] [--lockfile-path <path>] [--dry-run]");
    println!("  translation-freshness            Report docs/i18n translation freshness");
    println!("  doctor --release-gate             Include package-manager and marketplace checks");
    println!("  release --dry-run                 Local artifact dry-run (checksums + manifest)");
    println!("  release --version <tag>           Override version tag (default: Cargo.toml)");
    println!("  release --output-dir <path>       Output directory (default: release-artifacts/)");
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UpdateCliOptions {
    dry_run: bool,
    machine_only: bool,
    uninstall: bool,
    replace: bool,
    selected_runtimes: Option<Vec<String>>,
    primary_runtime: Option<String>,
}

fn parse_update_options(args: &[String]) -> Result<UpdateCliOptions, String> {
    let mut opts = UpdateCliOptions {
        dry_run: false,
        machine_only: false,
        uninstall: false,
        replace: false,
        selected_runtimes: None,
        primary_runtime: None,
    };
    let mut i = 1usize;
    while i < args.len() {
        match args[i].as_str() {
            "--dry-run" => opts.dry_run = true,
            "--machine-only" => opts.machine_only = true,
            "--uninstall" => opts.uninstall = true,
            "--replace" => opts.replace = true,
            "--selected-runtimes" => {
                i += 1;
                let value = args.get(i).ok_or_else(|| "--selected-runtimes requires a value".to_string())?;
                opts.selected_runtimes = Some(
                    value
                        .split(',')
                        .map(|item| item.trim().to_ascii_lowercase())
                        .filter(|item| !item.is_empty())
                        .collect(),
                );
            }
            "--primary-runtime" => {
                i += 1;
                let value = args.get(i).ok_or_else(|| "--primary-runtime requires a value".to_string())?;
                opts.primary_runtime = Some(value.trim().to_ascii_lowercase());
            }
            unknown if unknown.starts_with("--") => return Err(format!("unknown option '{unknown}'")),
            _ => {}
        }
        i += 1;
    }
    Ok(opts)
}

fn finalize_machine_options(
    config: &base::config::GalConfig,
    parsed: &UpdateCliOptions,
) -> Result<adapters::MachineUpdateOptions, String> {
    let mut opts = adapters::machine_options_from_config(config, parsed.dry_run).map_err(|e| e.to_string())?;
    opts.uninstall = parsed.uninstall;
    opts.replace = parsed.replace;
    if let Some(selected) = &parsed.selected_runtimes {
        opts.selected_runtimes = selected.clone();
    }
    if let Some(primary) = &parsed.primary_runtime {
        opts.primary_runtime = primary.clone();
    }
    Ok(opts)
}

/// Run `gal install` — Rust-native install flow (T-011).
fn cmd_install() -> ExitCode {
    use gal_engine::{config::GalConfig, install::run_install};

    let config = GalConfig::load();
    match run_install(&config) {
        Ok(report) => {
            let machine_opts = match finalize_machine_options(
                &config,
                &UpdateCliOptions {
                    dry_run: false,
                    machine_only: false,
                    uninstall: false,
                    replace: false,
                    selected_runtimes: None,
                    primary_runtime: None,
                },
            ) {
                Ok(opts) => opts,
                Err(e) => {
                    eprintln!("gal install: {e}");
                    return ExitCode::Error;
                }
            };
            if let Err(e) = adapters::run_machine_update(&machine_opts) {
                eprintln!("gal install: {e}");
                return ExitCode::Error;
            }
            println!("GAL installed to {}", report.canonical_root.display());
            println!("Mode: {}", report.mode);
            println!("Providers: {}", report.providers.join(", "));
            for w in &report.warnings {
                eprintln!("warning: {w}");
            }
            ExitCode::Success
        }
        Err(e) => {
            eprintln!("gal install: {e}");
            ExitCode::Error
        }
    }
}

/// Run `gal update` — re-render and re-project all surfaces (T-011/T-016).
fn cmd_update(args: &[String]) -> ExitCode {
    use gal_engine::{config::GalConfig, install::run_update};

    let parsed = match parse_update_options(args) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("gal update: {e}");
            return ExitCode::Usage;
        }
    };
    let config = GalConfig::load();
    if !parsed.machine_only {
        match run_update(&config) {
            Ok(report) => {
                println!("GAL updated at {}", report.canonical_root.display());
                println!("Mode: {}", report.mode);
                println!("Providers: {}", report.providers.join(", "));
                for w in &report.warnings {
                    eprintln!("warning: {w}");
                }
            }
            Err(e) => {
                eprintln!("gal update: {e}");
                return ExitCode::Error;
            }
        };
    }
    let machine_opts = match finalize_machine_options(&config, &parsed) {
        Ok(opts) => opts,
        Err(e) => {
            eprintln!("gal update: {e}");
            return ExitCode::Error;
        }
    };
    match adapters::run_machine_update(&machine_opts) {
        Ok(report) => {
            for warning in &report.warnings {
                eprintln!("warning: {}", warning.message);
            }
            ExitCode::Success
        }
        Err(e) => {
            eprintln!("gal update: {e}");
            ExitCode::Error
        }
    }
}

fn cmd_sync(args: &[String]) -> ExitCode {
    use gal_engine::config::GalConfig;

    let config = GalConfig::load();
    let target = args
        .iter()
        .skip(1)
        .find(|arg| !arg.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let opts = match adapters::sync_options_from_config(&config, target, false) {
        Ok(opts) => opts,
        Err(e) => {
            eprintln!("gal sync: {e}");
            return ExitCode::Error;
        }
    };
    match adapters::run_sync(&opts) {
        Ok(report) => {
            println!("gal sync: updated {} adapter file(s)", report.written_files.len());
            ExitCode::Success
        }
        Err(e) => {
            eprintln!("gal sync: {e}");
            ExitCode::Error
        }
    }
}

/// Run `gal uninstall` — remove canonical root and surfaces (T-011).
fn cmd_uninstall() -> ExitCode {
    use gal_engine::install::run_uninstall;

    match run_uninstall() {
        Ok(()) => {
            println!("GAL uninstalled.");
            ExitCode::Success
        }
        Err(e) => {
            eprintln!("gal uninstall: {e}");
            ExitCode::Error
        }
    }
}

/// Run `gal doctor [--dry-run] [--release-gate]` — read-only health checks (T-012).
fn cmd_doctor(args: &[String]) -> ExitCode {
    use gal_engine::doctor::{run_doctor, DoctorOptions};

    let dry_run = args.iter().any(|a| a == "--dry-run");
    let release_gate = args.iter().any(|a| a == "--release-gate");

    let opts = DoctorOptions { dry_run, release_gate };
    let mut report = run_doctor(&opts);

    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if let Some(check) = mcp::McpProjectionHealthCheck::from_standard_path() {
        report.findings.extend(check.check());
    }
    report.findings.extend(
        setup::health::SetupHealthCheck {
            repo_root: repo_root.clone(),
        }
        .check(),
    );
    if let Some(home) = base::paths::user_home() {
        let shared_skills_root = home.join(".agents").join("skills");
        report
            .findings
            .extend(adapters::SkillsProjectionHealthCheck::with_path(shared_skills_root).check());
    }

    if report.findings.is_empty() {
        println!("gal doctor: all checks passed.");
    } else {
        for f in &report.findings {
            println!("{f}");
        }
    }

    if report.has_errors() {
        ExitCode::Error
    } else {
        ExitCode::Success
    }
}

/// Run `gal release [--dry-run] [--version <tag>] [--output-dir <dir>]` (T-014).
///
/// Produces `checksums.txt` and `artifact-manifest.json` in the output directory.
/// Cosign signing is CI-only (OIDC); locally a placeholder is written instead.
fn cmd_release(args: &[String]) -> ExitCode {
    use gal_engine::release::{run_release, AssetSpec, ReleaseOptions};
    use std::path::PathBuf;

    // Parse flags: --version <tag>, --output-dir <path>, --dry-run (implied always)
    let mut version = format!("v{}", env!("CARGO_PKG_VERSION"));
    let mut output_dir = PathBuf::from("release-artifacts");
    let mut extra_assets: Vec<PathBuf> = Vec::new();

    let mut i = 1usize; // skip "release"
    while i < args.len() {
        match args[i].as_str() {
            "--version" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    version = v.clone();
                } else {
                    eprintln!("gal release: --version requires a value");
                    return ExitCode::Usage;
                }
            }
            "--output-dir" => {
                i += 1;
                if let Some(d) = args.get(i) {
                    output_dir = PathBuf::from(d);
                } else {
                    eprintln!("gal release: --output-dir requires a value");
                    return ExitCode::Usage;
                }
            }
            "--asset" => {
                // Accept --asset <path> for CI invocation with pre-built binaries.
                i += 1;
                if let Some(p) = args.get(i) {
                    extra_assets.push(PathBuf::from(p));
                } else {
                    eprintln!("gal release: --asset requires a path");
                    return ExitCode::Usage;
                }
            }
            "--dry-run" => {} // always dry-run locally; flag is accepted but implied
            unknown => {
                eprintln!("gal release: unknown option '{unknown}'");
                return ExitCode::Usage;
            }
        }
        i += 1;
    }

    // Build asset specs from explicit --asset paths (CI) or empty (local dev dry-run).
    let assets: Vec<AssetSpec> = extra_assets
        .into_iter()
        .map(|path| {
            // Infer platform/arch/kind from the canonical filename convention:
            // gal-<version>-<platform>-<arch>[.exe|.zip|.tar.gz]
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let (platform, architecture, kind) = parse_asset_name(&name, &version);
            AssetSpec {
                name,
                platform,
                architecture,
                kind,
                contains: vec![],
                path,
            }
        })
        .collect();

    match run_release(ReleaseOptions { version: version.clone(), output_dir: output_dir.clone(), assets }) {
        Ok(result) => {
            println!("gal release: artifacts written to {}", output_dir.display());
            println!("  checksums.txt:        {}", result.checksums_path.display());
            println!("  artifact-manifest.json: {}", result.manifest_path.display());
            if result.cosign_skipped {
                println!("  cosign: SKIPPED (no OIDC locally) — placeholder written");
                println!("  To sign, run this in CI with OIDC token available.");
            }
            println!("  version: {}", result.manifest.version);
            println!("  assets:  {}", result.manifest.assets.len());
            ExitCode::Success
        }
        Err(e) => {
            eprintln!("gal release: {e}");
            ExitCode::Error
        }
    }
}

/// Run `gal mcp [update]` — write per-provider MCP config files (T-011, R-02).
///
/// Usage:
///   gal mcp update [--manifest <path>]
///   gal mcp         (implies `update`)
fn cmd_mcp(args: &[String]) -> ExitCode {
    use mcp::run_mcp_update;

    // Accept `gal mcp` or `gal mcp update`; reject unknown subcommands.
    let sub = args.iter().skip(1).find(|a| !a.starts_with("--")).map(|s| s.as_str());
    match sub {
        None | Some("update") => {}
        Some(other) => {
            eprintln!("gal mcp: unknown subcommand '{other}'. Use: gal mcp update");
            return ExitCode::Usage;
        }
    }

    // Resolve manifest path: --manifest <path> or default ~/.gal/managed.mcp.json
    let manifest_path = {
        let mut path: Option<std::path::PathBuf> = None;
        let mut i = 1usize;
        while i < args.len() {
            if args[i] == "--manifest" {
                i += 1;
                if let Some(p) = args.get(i) {
                    path = Some(std::path::PathBuf::from(p));
                } else {
                    eprintln!("gal mcp: --manifest requires a path");
                    return ExitCode::Usage;
                }
            }
            i += 1;
        }
        path.or_else(|| {
            base::paths::gal_home().map(|h| {
                h.join("plugins").join("gal-core").join("mcp.json")
            })
        })
    };

    let manifest_path = match manifest_path {
        Some(p) => p,
        None => {
            eprintln!("gal mcp: could not resolve home directory");
            return ExitCode::Error;
        }
    };

    if !manifest_path.exists() {
        eprintln!("gal mcp: manifest not found: {}", manifest_path.display());
        eprintln!("         Run `gal install` first to set up the plugin tree.");
        return ExitCode::Error;
    }

    match run_mcp_update(&manifest_path) {
        Ok(report) => {
            println!(
                "gal mcp: updated {} provider(s), {} server(s)",
                report.providers_updated.len(),
                report.servers_written
            );
            if !report.providers_updated.is_empty() {
                println!("  providers: {}", report.providers_updated.join(", "));
            }
            for w in &report.warnings {
                eprintln!("  warning: {w}");
            }
            ExitCode::Success
        }
        Err(e) => {
            eprintln!("gal mcp: {e}");
            ExitCode::Error
        }
    }
}

/// Run `gal setup [...]` — machine-setup orchestration (T-018, R-04).
///
/// Ports `Setup-Machine.{ps1,sh}`: AGY legacy pre-cleanup, machine surfaces
/// (library call), MCP refresh (library call), then install orchestration via
/// the Rust install path (`gal_engine::install`, repointed at R-05/T-024).
fn cmd_setup(args: &[String]) -> ExitCode {
    let mut opts = setup::SetupOptions::default();
    let mut i = 1usize;
    while i < args.len() {
        match args[i].as_str() {
            "--uninstall" => opts.uninstall = true,
            "--purge" => opts.purge = true,
            "--confirm-purge" => opts.confirm_purge = true,
            "--replace" => opts.replace = true,
            "--dry-run" => opts.dry_run = true,
            "--check" => opts.check = true,
            "--reconfigure" => opts.reconfigure = true,
            "--bootstrap-install" => opts.bootstrap_install = true,
            "--tools" => opts.tools = true,
            "--tool" => {
                i += 1;
                match args.get(i) {
                    Some(value) => {
                        let mut list = opts.tool.take().unwrap_or_default();
                        list.extend(
                            value
                                .split(',')
                                .map(|item| item.trim().to_string())
                                .filter(|item| !item.is_empty()),
                        );
                        opts.tool = Some(list);
                    }
                    None => {
                        eprintln!("gal setup: --tool requires a value");
                        return ExitCode::Usage;
                    }
                }
            }
            "--selected-runtimes" => {
                i += 1;
                match args.get(i) {
                    Some(value) => {
                        opts.selected_runtimes = Some(
                            value
                                .split(',')
                                .map(|item| item.trim().to_string())
                                .filter(|item| !item.is_empty())
                                .collect(),
                        );
                    }
                    None => {
                        eprintln!("gal setup: --selected-runtimes requires a value");
                        return ExitCode::Usage;
                    }
                }
            }
            "--primary-runtime" => {
                i += 1;
                match args.get(i) {
                    Some(value) => opts.primary_runtime = Some(value.trim().to_string()),
                    None => {
                        eprintln!("gal setup: --primary-runtime requires a value");
                        return ExitCode::Usage;
                    }
                }
            }
            unknown => {
                eprintln!("gal setup: unknown option '{unknown}'");
                return ExitCode::Usage;
            }
        }
        i += 1;
    }

    let mut stdout = std::io::stdout();
    let interactive = std::io::IsTerminal::is_terminal(&std::io::stdin());
    let result = if interactive {
        let mut prompter = setup::session::StdinPrompter;
        setup::run_setup(&opts, &mut prompter, &mut stdout)
    } else {
        let mut prompter = setup::session::NonInteractivePrompter;
        setup::run_setup(&opts, &mut prompter, &mut stdout)
    };

    match result {
        Ok(outcome) => {
            if outcome.exit_code == 0 {
                ExitCode::Success
            } else {
                // Propagate the legacy script's exit code verbatim (C-3).
                std::process::exit(outcome.exit_code)
            }
        }
        Err(setup::SetupError::Usage(message)) => {
            eprintln!("gal setup: {message}");
            ExitCode::Usage
        }
        Err(e) => {
            eprintln!("gal setup: {e}");
            ExitCode::Error
        }
    }
}

fn cmd_filter_transform(args: &[String], smudge: bool) -> ExitCode {
    use std::io::{Read, Write};

    let _ = args;
    let repo_root = match std::env::current_dir() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("gal {}: {e}", if smudge { "smudge" } else { "clean" });
            return ExitCode::Error;
        }
    };

    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("gal {}: failed to read stdin: {e}", if smudge { "smudge" } else { "clean" });
        return ExitCode::Error;
    }

    let result = if smudge {
        gal_engine::git_filters::run_smudge(&input, &repo_root)
    } else {
        gal_engine::git_filters::run_clean(&input, &repo_root)
    };

    match result {
        Ok(result) => {
            if let Err(e) = std::io::stdout().write_all(result.output.as_bytes()) {
                eprintln!("gal {}: failed to write stdout: {e}", if smudge { "smudge" } else { "clean" });
                return ExitCode::Error;
            }
            for warning in result.warnings {
                eprintln!("{warning}");
            }
            ExitCode::Success
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::Error
        }
    }
}

fn cmd_init_repo(args: &[String]) -> ExitCode {
    let options = match init_repo::parse_init_repo_options(args) {
        Ok(options) => options,
        Err(e) => {
            eprintln!("gal init-repo: {e}");
            return ExitCode::Usage;
        }
    };

    match init_repo::run_init_repo(&options) {
        Ok(report) => {
            println!("Initialized repo context in: {}", report.target_path.display());
            println!("- Created: .dev/project.md");
            println!("- Created: .dev/state.md");
            println!("- Ensured: docs/plans/");
            println!("- Generated: .github/copilot-instructions.md");
            println!("- Generated: GEMINI.md");
            println!("- Generated: CLAUDE.md");
            println!("- Generated: AGENTS.md");
            println!("- Next: review .dev/project.md, fill in summary fields, then run /gal status");
            if !report.source_docs.is_empty() {
                println!();
                println!("Adopt-existing: found {} source document(s).", report.source_docs.len());
                println!("Review .dev/project.md and fill in summaries from discovered docs.");
            }
            if !report.tech_hints.is_empty() {
                println!("Detected tech stack: {}", report.tech_hints.join(", "));
            }
            ExitCode::Success
        }
        Err(e) => {
            eprintln!("gal init-repo: {e}");
            ExitCode::Error
        }
    }
}

fn cmd_resolve_catalog(args: &[String]) -> ExitCode {
    let mut catalog_path = PathBuf::from("plugins/catalog.json");
    let mut config_path = base::paths::machine_config_path().unwrap_or_else(|| PathBuf::from(".gal/config/config.json"));
    let mut lockfile_path = base::paths::gal_home()
        .map(|home| home.join("state").join("plugins.lock.json"))
        .unwrap_or_else(|| PathBuf::from(".gal/state/plugins.lock.json"));
    let mut dry_run = false;

    let mut i = 1usize;
    while i < args.len() {
        match args[i].as_str() {
            "--catalog-path" => {
                i += 1;
                if let Some(value) = args.get(i) {
                    catalog_path = PathBuf::from(value);
                } else {
                    eprintln!("gal resolve-catalog: --catalog-path requires a value");
                    return ExitCode::Usage;
                }
            }
            "--config-path" => {
                i += 1;
                if let Some(value) = args.get(i) {
                    config_path = PathBuf::from(value);
                } else {
                    eprintln!("gal resolve-catalog: --config-path requires a value");
                    return ExitCode::Usage;
                }
            }
            "--lockfile-path" => {
                i += 1;
                if let Some(value) = args.get(i) {
                    lockfile_path = PathBuf::from(value);
                } else {
                    eprintln!("gal resolve-catalog: --lockfile-path requires a value");
                    return ExitCode::Usage;
                }
            }
            "--dry-run" => dry_run = true,
            unknown => {
                eprintln!("gal resolve-catalog: unknown option '{unknown}'");
                return ExitCode::Usage;
            }
        }
        i += 1;
    }

    match gal_engine::catalog::run_resolve_catalog(&gal_engine::catalog::ResolveCatalogOptions {
        catalog_path,
        config_path,
        lockfile_path: lockfile_path.clone(),
        dry_run,
    }) {
        Ok(result) => {
            if dry_run {
                println!("--- DRY RUN ---");
            } else {
                println!("Lockfile written: {}", lockfile_path.display());
            }
            println!("Profile: {}", result.profile_name);
            let ids = result
                .resolved_plugins
                .iter()
                .filter_map(|plugin| plugin.get("pluginId").and_then(serde_json::Value::as_str))
                .collect::<Vec<_>>()
                .join(", ");
            println!("Resolved plugins: {ids}");
            println!("Validation errors: {}", result.errors.len());
            println!("Drift detected: {}", result.drift_detected);
            for detail in result.drift_details {
                println!("  Drift: {detail}");
            }
            if dry_run {
                println!("Lockfile preview:");
                println!("{}", serde_json::to_string_pretty(&result.lockfile).unwrap_or_default());
            }
            if result.errors.is_empty() {
                ExitCode::Success
            } else {
                ExitCode::Error
            }
        }
        Err(e) => {
            eprintln!("gal resolve-catalog: {e}");
            ExitCode::Error
        }
    }
}

fn cmd_translation_freshness() -> ExitCode {
    let repo_root = match std::env::current_dir() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("gal translation-freshness: {e}");
            return ExitCode::Error;
        }
    };

    let report = gal_engine::translation::run_translation_freshness(
        &repo_root,
        &["README.md", "docs/manual.md"],
    );
    if report.rows.is_empty() {
        println!("No docs/i18n/ tree found; nothing to check.");
        return ExitCode::Success;
    }

    println!("{:<34} {:<8} {:<9} NOTE", "DOC", "LANG", "STATUS");
    for row in report.rows {
        println!("{:<34} {:<8} {:<9} {}", row.doc, row.lang, row.status, row.note);
    }
    println!(
        "Summary: current={}  stale={}  missing={}",
        report.current, report.stale, report.missing
    );
    ExitCode::Success
}

/// Infer (platform, architecture, ArtifactKind) from a canonical asset filename.
/// Falls back to ("unknown", "unknown", Binary) when the name does not match the
/// convention — never panics.
fn parse_asset_name(name: &str, _version: &str) -> (String, String, gal_engine::release::ArtifactKind) {
    use gal_engine::release::ArtifactKind;

    let kind = if name.ends_with(".zip") || name.ends_with(".tar.gz") {
        ArtifactKind::Archive
    } else {
        ArtifactKind::Binary
    };

    // Strip known suffixes to expose the platform-arch portion.
    let stem = name
        .trim_end_matches(".tar.gz")
        .trim_end_matches(".zip")
        .trim_end_matches(".exe");

    // Expected: gal-<version>-<platform>-<arch>
    let parts: Vec<&str> = stem.split('-').collect();
    if parts.len() >= 2 {
        let arch = parts[parts.len() - 1].to_string();
        let platform = parts[parts.len() - 2].to_string();
        (platform, arch, kind)
    } else {
        ("unknown".to_string(), "unknown".to_string(), kind)
    }
}

/// Run `gal commit-msg` — git commit-msg hook + message generator.
///
/// Modes:
/// - `gal commit-msg --print`  → write a generated message to stdout (for the
///   `git-commits` skill / `git-commit-msg` command). Replaces the retired
///   `Get-StagedCommitMessage.ps1` / `get-staged-commit-message.sh`.
/// - `gal commit-msg <file>`   → git commit-msg hook. Non-destructive: an
///   existing author message is preserved; only a blank message is filled.
fn cmd_commit_msg(args: &[String]) -> ExitCode {
    use gal_engine::commit_msg::{fill_commit_msg_file, generate_commit_message, CommitMsgResult};

    let entries = get_staged_entries();

    // --print mode: emit the generated message (or a friendly note) to stdout.
    if args.iter().any(|a| a == "--print" || a == "--generate") {
        match generate_commit_message(&entries) {
            Some(msg) => {
                println!("{msg}");
                return ExitCode::Success;
            }
            None => {
                println!("No changes staged for commit.");
                return ExitCode::Success;
            }
        }
    }

    // Hook mode: first positional arg is the commit message file path.
    let msg_path_str = match args.iter().skip(1).find(|a| !a.starts_with("--")) {
        Some(p) => p.clone(),
        None => {
            eprintln!("gal commit-msg: missing message file argument");
            eprintln!("usage: gal commit-msg <path-to-commit-message-file>");
            eprintln!("       gal commit-msg --print");
            return ExitCode::Usage;
        }
    };

    let msg_path = Path::new(&msg_path_str);
    if !msg_path.exists() {
        eprintln!("gal commit-msg: message file not found: {msg_path_str}");
        return ExitCode::Error;
    }

    match fill_commit_msg_file(msg_path, &entries) {
        Ok(CommitMsgResult::NoOp | CommitMsgResult::Updated) => ExitCode::Success,
        Err(e) => {
            eprintln!("gal commit-msg: {e}");
            ExitCode::Error
        }
    }
}

/// Retrieve staged entries (status + path, rename-aware) from git.
/// Returns an empty list when git is unavailable or nothing is staged.
fn get_staged_entries() -> Vec<gal_engine::commit_msg::StagedEntry> {
    let output = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-status", "--find-renames"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            gal_engine::commit_msg::parse_name_status(&text)
        }
        _ => Vec::new(), // git unavailable or not a repo → treat as empty staging
    }
}

fn run(args: &[String]) -> ExitCode {
    match classify_args(args) {
        Action::Version => {
            println!("gal {}", env!("CARGO_PKG_VERSION"));
            ExitCode::Success
        }
        Action::Help => {
            print_help();
            ExitCode::Success
        }
        Action::MissingCommand => {
            print_help();
            ExitCode::Usage
        }
        // T-011: wired install/update/uninstall
        Action::NotWired(CommandKind::Install) => cmd_install(),
        Action::NotWired(CommandKind::Update) => cmd_update(args),
        Action::NotWired(CommandKind::Sync) => cmd_sync(args),
        Action::NotWired(CommandKind::Uninstall) => cmd_uninstall(),
        // T-012: wired doctor
        Action::NotWired(CommandKind::Doctor) => cmd_doctor(args),
        // T-013: wired commit-msg (R5, optional)
        Action::NotWired(CommandKind::CommitMsg) => cmd_commit_msg(args),
        // T-014: wired release artifact generation
        Action::NotWired(CommandKind::Release) => cmd_release(args),
        // T-011: wired gal mcp (R-02)
        Action::NotWired(CommandKind::Mcp) => cmd_mcp(args),
        // T-018: wired gal setup (R-04)
        Action::NotWired(CommandKind::Setup) => cmd_setup(args),
        // T-025: wired git clean/smudge filters
        Action::NotWired(CommandKind::Clean) => cmd_filter_transform(args, false),
        Action::NotWired(CommandKind::Smudge) => cmd_filter_transform(args, true),
        // T-027: wired init-repo
        Action::NotWired(CommandKind::InitRepo) => cmd_init_repo(args),
        // T-028: wired resolve-catalog
        Action::NotWired(CommandKind::ResolveCatalog) => cmd_resolve_catalog(args),
        // T-030: wired translation freshness
        Action::NotWired(CommandKind::TranslationFreshness) => cmd_translation_freshness(),
        // Not yet wired (dispatch-script, etc.)
        Action::NotWired(cmd) => {
            eprintln!("gal {}: not wired", cmd.as_str());
            ExitCode::NotWired
        }
        Action::UnknownCommand(cmd) => {
            eprintln!("gal: unknown command '{cmd}'. Run `gal --help`.");
            ExitCode::Usage
        }
    }
}

fn main() -> ProcessExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(&args).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_flag_succeeds() {
        assert_eq!(run(&["--version".to_string()]), ExitCode::Success);
    }

    #[test]
    fn help_flag_succeeds() {
        assert_eq!(run(&["--help".to_string()]), ExitCode::Success);
    }

    #[test]
    fn no_args_is_usage() {
        assert_eq!(run(&[]), ExitCode::Usage);
    }

    #[test]
    fn unknown_command_is_usage() {
        assert_eq!(run(&["frobnicate".to_string()]), ExitCode::Usage);
    }

    // T-011: install/update/uninstall are now wired (return Success or Error, not NotWired)
    #[test]
    fn install_is_wired_not_not_wired() {
        let result = run(&["install".to_string()]);
        assert_ne!(
            result,
            ExitCode::NotWired,
            "install must be wired (T-011) — should return Success or Error, not NotWired"
        );
    }

    #[test]
    fn update_is_wired_not_not_wired() {
        let result = run(&["update".to_string()]);
        assert_ne!(result, ExitCode::NotWired, "update must be wired (T-011)");
    }

    #[test]
    fn sync_is_wired_not_not_wired() {
        let result = run(&["sync".to_string()]);
        assert_ne!(result, ExitCode::NotWired, "sync must be wired");
    }

    #[test]
    fn uninstall_is_wired_not_not_wired() {
        let result = run(&["uninstall".to_string()]);
        assert_ne!(result, ExitCode::NotWired, "uninstall must be wired (T-011)");
    }

    // T-012: doctor is wired
    #[test]
    fn doctor_is_wired_not_not_wired() {
        let result = run(&["doctor".to_string()]);
        assert_ne!(result, ExitCode::NotWired, "doctor must be wired (T-012)");
    }

    #[test]
    fn doctor_dry_run_is_wired() {
        let result = run(&["doctor".to_string(), "--dry-run".to_string()]);
        assert_ne!(result, ExitCode::NotWired, "doctor --dry-run must be wired (T-012)");
    }

    #[test]
    fn doctor_command_remains_wired_after_domain_health_aggregation() {
        let result = run(&["doctor".to_string()]);
        assert_ne!(result, ExitCode::NotWired, "doctor must stay wired after T-031 aggregation");
    }

    // T-013: commit-msg is wired
    #[test]
    fn commit_msg_without_arg_returns_usage() {
        let result = run(&["commit-msg".to_string()]);
        assert_eq!(result, ExitCode::Usage, "commit-msg with no file arg should be Usage");
    }

    // T-018: setup is wired. Use --purge (fails fast at the gating check)
    // so the test never reaches filesystem or script side effects.
    #[test]
    fn setup_is_wired_and_purge_gating_returns_usage() {
        let result = run(&["setup".to_string(), "--purge".to_string()]);
        assert_eq!(
            result,
            ExitCode::Usage,
            "setup must be wired (T-018) and --purge without --uninstall must be a usage error"
        );
    }

    #[test]
    fn setup_unknown_option_is_usage() {
        let result = run(&["setup".to_string(), "--frobnicate".to_string()]);
        assert_eq!(result, ExitCode::Usage);
    }

    #[test]
    fn dispatch_script_is_still_not_wired() {
        assert_eq!(
            run(&["dispatch-script".to_string()]),
            ExitCode::NotWired,
            "dispatch-script should remain not wired"
        );
    }

    #[test]
    fn exit_code_values_are_correct() {
        assert_eq!(ExitCode::Success as u8, 0);
        assert_eq!(ExitCode::Error as u8, 1);
        assert_eq!(ExitCode::NotWired as u8, 2);
        assert_eq!(ExitCode::Usage as u8, 64);
    }
}
