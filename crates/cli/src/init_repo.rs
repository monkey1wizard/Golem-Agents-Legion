use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitRepoOptions {
    pub target_path: PathBuf,
    pub project_name: Option<String>,
    pub blank: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDoc {
    pub path: String,
    pub doc_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitRepoReport {
    pub target_path: PathBuf,
    pub source_docs: Vec<SourceDoc>,
    pub tech_hints: Vec<String>,
    /// The renderer's write/removal outcome for the repo-local adapters,
    /// captured from `gal::render::run_sync` on both the existing-repo regen
    /// path and the fresh bootstrap path — previously discarded.
    pub adapter_report: crate::gal::render::ProjectionReport,
}

pub fn parse_init_repo_options(args: &[String]) -> Result<InitRepoOptions, String> {
    let mut blank = false;

    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "--blank" => blank = true,
            "--force" => {
                return Err(concat!(
                    "`--force` has been removed, `gal init` now refuses a repository ",
                    "that already holds `.dev/project.md` or `.dev/state.md`, ",
                    "running `gal render-adapters` regenerates the repo-local adapters, ",
                    "and deleting both `.dev` files then running `gal init` starts ",
                    "over from the templates."
                )
                .to_string());
            }
            unknown => {
                return Err(format!("unknown option '{unknown}'"));
            }
        }
    }

    let target_path = std::env::current_dir().map_err(|e| e.to_string())?;

    Ok(InitRepoOptions {
        target_path,
        project_name: None,
        blank,
    })
}

/// Strip the Windows verbatim `\\?\` prefix (and map `\\?\UNC\` back to `\\`)
/// that `std::fs::canonicalize` adds. No-op on a string without the prefix,
/// so it is safe on all platforms. Mirrors `commands/refresh.rs`'s helper of
/// the same name — kept local rather than shared to avoid a cross-module
/// dependency for a two-branch string match.
fn strip_verbatim_prefix(s: &str) -> String {
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        s.to_string()
    }
}

pub fn run_init_repo(opts: &InitRepoOptions) -> Result<InitRepoReport, String> {
    if !opts.target_path.exists() {
        return Err(format!(
            "Target path does not exist: {}",
            opts.target_path.display()
        ));
    }

    let resolved_target = opts.target_path.canonicalize().map_err(|e| {
        format!(
            "Failed to resolve target path {}: {e}",
            opts.target_path.display()
        )
    })?;
    // `canonicalize()` adds the Windows verbatim `\\?\` prefix. `render.rs`'s
    // `repo_skill_inventory()` strips this same prefix-carrying `repo_root`
    // against a non-verbatim `source_root` (from `env::current_dir()`), so an
    // un-normalized `resolved_target` silently breaks `strip_prefix` there —
    // `## Repo Skills` renders empty on every Windows self-hosted repo. Strip
    // it here, at the single point `repo_root` is derived, matching the same
    // `strip_verbatim_prefix` pattern already used in `commands/refresh.rs`.
    let resolved_target = PathBuf::from(strip_verbatim_prefix(&resolved_target.to_string_lossy()));

    let dev_dir = resolved_target.join(".dev");
    let project_target_path = dev_dir.join("project.md");
    let state_target_path = dev_dir.join("state.md");

    match (project_target_path.is_file(), state_target_path.is_file()) {
        (true, true) => {
            return Err(concat!(
                "this repository is already initialized (.dev/project.md exists).\n",
                "  To regenerate AGENTS.md and CLAUDE.md from .dev/project.md, run: gal render-adapters\n",
                "  To start over from the templates, delete .dev/project.md and .dev/state.md, then run gal init again."
            )
            .to_string());
        }
        (true, false) => {
            return Err(concat!(
                "this repository is half-initialized: .dev/project.md exists but .dev/state.md is missing.\n",
                "  Restore .dev/state.md from version control, or delete .dev/project.md and run gal init again.\n",
                "  gal init does not overwrite .dev/project.md."
            )
            .to_string());
        }
        (false, true) => {
            return Err(concat!(
                "this repository is half-initialized: .dev/state.md exists but .dev/project.md is missing.\n",
                "  Restore .dev/project.md from version control, or delete .dev/state.md and run gal init again.\n",
                "  gal init does not overwrite .dev/state.md."
            )
            .to_string());
        }
        (false, false) => {}
    }

    let project_name = opts.project_name.clone().unwrap_or_else(|| {
        resolved_target
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    });

    let source_root = resolve_gal_source_root()?;
    let templates_root = source_root.join("templates");
    let project_template_path = templates_root.join("project.md");
    let state_template_path = templates_root.join("state.md");
    if !project_template_path.is_file() {
        return Err(format!(
            "Missing template: {}",
            project_template_path.display()
        ));
    }
    if !state_template_path.is_file() {
        return Err(format!(
            "Missing template: {}",
            state_template_path.display()
        ));
    }

    let plans_dir = resolved_target.join(".dev").join("plans");
    fs::create_dir_all(&dev_dir)
        .map_err(|e| format!("Failed to create {}: {e}", dev_dir.display()))?;
    fs::create_dir_all(&plans_dir)
        .map_err(|e| format!("Failed to create {}: {e}", plans_dir.display()))?;

    let (source_docs, tech_hints) = if opts.blank {
        (Vec::new(), Vec::new())
    } else {
        scan_adopt_existing(&resolved_target)?
    };

    let mut project_content = fs::read_to_string(&project_template_path)
        .map_err(|e| format!("Failed to read {}: {e}", project_template_path.display()))?;
    project_content = project_content.replace("# [Project Name]", &format!("# {project_name}"));
    if !source_docs.is_empty() {
        let mut doc_table = String::from("| Path | Type | Notes |\n| --- | --- | --- |\n");
        for doc in &source_docs {
            doc_table.push_str(&format!("| `{}` | {} | |\n", doc.path, doc.doc_type));
        }
        project_content =
            project_content.replace("[Index of source docs...]", doc_table.trim_end());
    }
    if !tech_hints.is_empty() {
        project_content = project_content.replace(
            "[Language / framework / major libs / infrastructure]",
            &tech_hints.join(", "),
        );
    }

    let state_content = fs::read_to_string(&state_template_path)
        .map_err(|e| format!("Failed to read {}: {e}", state_template_path.display()))?;

    fs::write(&project_target_path, project_content)
        .map_err(|e| format!("Failed to write {}: {e}", project_target_path.display()))?;
    fs::write(&state_target_path, state_content)
        .map_err(|e| format!("Failed to write {}: {e}", state_target_path.display()))?;

    let sync_options = crate::gal::render::sync_options_from(resolved_target.clone(), false)
        .map_err(|e| e.to_string())?;
    let adapter_report = crate::gal::render::run_sync(&sync_options).map_err(|e| e.to_string())?;
    graphify_auto_init(&resolved_target)?;

    Ok(InitRepoReport {
        target_path: resolved_target,
        source_docs,
        tech_hints,
        adapter_report,
    })
}

/// Resolve the GAL source root — the `gal-core`-equivalent directory that holds
/// `templates/`, `skills/`, `agents/`, `commands/`. Handles both layouts by
/// reusing the same resolution helpers `gal refresh` uses:
///   1. a repo checkout — an ancestor whose `plugins/gal-core` (or the level
///      itself, flat) looks like a source root, and
///   2. an installed binary — flat exe-side layout or FHS `share/gal` beside
///      the binary.
///
/// A channel-installed machine (no checkout) has no `plugins/gal-core` ancestor,
/// so a cwd-only walk returned "Could not locate the GAL repo root" and blocked
/// `gal init` on every installed machine. In both layouts `templates/` is a
/// direct child of the returned root. (The Cargo bare-binary / embedded-payload
/// leg is wired via the shared refresh resolver when embedded support lands.)
fn resolve_gal_source_root() -> Result<PathBuf, String> {
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(root) = gal_engine::render::resolve_source_from_cwd(&cwd) {
            return Ok(root);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        // Canonicalize first so a symlinked entry point (e.g. winget's
        // WinGet\Links\gal.exe) resolves to the real install dir.
        let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
        if let Some(exe_dir) = exe.parent() {
            if let Some(root) = gal_engine::render::resolve_source_from_exe_dir(exe_dir) {
                return Ok(root);
            }
        }
    }
    // Cargo bare-binary case: materialize the build-time embedded payload
    // (shared with `gal refresh`) and use it as the source root.
    if let Some(root) = gal_engine::render::materialize_embedded_source() {
        return Ok(root);
    }
    Err(
        "Could not locate the GAL source root — neither a repo checkout (an \
         ancestor containing plugins/gal-core) nor an installed payload (flat \
         layout or share/gal beside the binary) was found. Reinstall via a \
         packaging channel or run from a GAL checkout."
            .to_string(),
    )
}

fn scan_adopt_existing(target: &Path) -> Result<(Vec<SourceDoc>, Vec<String>), String> {
    let mut source_docs = Vec::new();
    for name in ["README.md", "README", "README.rst", "README.txt"] {
        let path = target.join(name);
        if path.is_file() {
            source_docs.push(SourceDoc {
                path: name.to_string(),
                doc_type: "README".to_string(),
            });
        }
    }

    let docs_dir = target.join("docs");
    if docs_dir.is_dir() {
        for file in collect_files(&docs_dir, &["md", "rst", "txt"])? {
            source_docs.push(SourceDoc {
                path: relative_path(target, &file),
                doc_type: "docs".to_string(),
            });
        }
    }

    for adr_dir in ["docs/adr", "docs/ADR", "adr", "ADR"] {
        let path = target.join(adr_dir);
        if path.is_dir() {
            for file in collect_files(&path, &["md"])? {
                source_docs.push(SourceDoc {
                    path: relative_path(target, &file),
                    doc_type: "ADR".to_string(),
                });
            }
        }
    }

    let mut tech_hints = Vec::new();
    for (pattern, tech) in [
        ("*.csproj", ".NET"),
        ("*.sln", ".NET"),
        ("package.json", "Node.js"),
        ("go.mod", "Go"),
        ("Cargo.toml", "Rust"),
        ("pyproject.toml", "Python"),
        ("requirements.txt", "Python"),
        ("Gemfile", "Ruby"),
        ("pom.xml", "Java/Maven"),
        ("build.gradle", "Java/Gradle"),
        ("build.gradle.kts", "Java/Gradle"),
    ] {
        if has_match_within_depth(target, pattern, 2)? && !tech_hints.contains(&tech.to_string()) {
            tech_hints.push(tech.to_string());
        }
    }

    Ok((source_docs, tech_hints))
}

fn collect_files(root: &Path, extensions: &[&str]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    walk_files(root, &mut files, extensions)
        .map_err(|e| format!("Failed to scan {}: {e}", root.display()))?;
    files.sort();
    Ok(files)
}

fn walk_files(root: &Path, files: &mut Vec<PathBuf>, extensions: &[&str]) -> std::io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            walk_files(&path, files, extensions)?;
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| extensions.contains(&ext))
            .unwrap_or(false)
        {
            files.push(path);
        }
    }
    Ok(())
}

fn has_match_within_depth(root: &Path, pattern: &str, max_depth: usize) -> Result<bool, String> {
    fn walk(root: &Path, pattern: &str, max_depth: usize, depth: usize) -> std::io::Result<bool> {
        if depth > max_depth {
            return Ok(false);
        }
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                if walk(&path, pattern, max_depth, depth + 1)? {
                    return Ok(true);
                }
            } else if pattern_matches(&path, pattern) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    walk(root, pattern, max_depth, 0).map_err(|e| format!("Failed to scan {}: {e}", root.display()))
}

fn pattern_matches(path: &Path, pattern: &str) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    match pattern {
        "*.csproj" => name.ends_with(".csproj"),
        "*.sln" => name.ends_with(".sln"),
        other => name == other,
    }
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn graphify_auto_init(repo_path: &Path) -> Result<(), String> {
    let graph_report_path = repo_path.join("graphify-out").join("GRAPH_REPORT.md");
    if !graph_report_path.is_file() {
        return Ok(());
    }

    let graph_version_path = repo_path
        .join("graphify-out")
        .join("GAL_GRAPHIFY_VERSION.txt");
    if graph_version_path.is_file() && command_available("graphify") {
        if let Some(current_version) = get_graphify_version() {
            let stamped_version = fs::read_to_string(&graph_version_path).unwrap_or_default();
            let report_info = fs::metadata(&graph_report_path)
                .map_err(|e| format!("Failed to stat {}: {e}", graph_report_path.display()))?;
            let stamp_info = fs::metadata(&graph_version_path)
                .map_err(|e| format!("Failed to stat {}: {e}", graph_version_path.display()))?;
            if !stamped_version.trim().is_empty()
                && stamped_version.trim() != current_version
                && report_info.modified().ok() <= stamp_info.modified().ok()
            {
                eprintln!("Warning: graphify version changed (report: {}, installed: {}). Refresh graphify artifacts manually if you want updated graph context.", stamped_version.trim(), current_version);
            }
        }
    }

    if !graph_version_path.is_file() && command_available("graphify") {
        if let Some(current_version) = get_graphify_version() {
            fs::write(&graph_version_path, format!("{current_version}\n"))
                .map_err(|e| format!("Failed to write {}: {e}", graph_version_path.display()))?;
        }
    }

    Ok(())
}

fn command_available(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok()
}

fn get_graphify_version() -> Option<String> {
    let output = Command::new("graphify").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .and_then(|text| text.lines().next().map(|line| line.trim().to_string()))
        .filter(|line| !line.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Sentinel `.dev/project.md` for the safe-regen tests. Distinct from the
    /// template ("My Existing Project"), and section-complete: the renderer
    /// fail-closes when any required H2 section is missing, so a regen fixture
    /// must carry all eight.
    fn sentinel_project_md() -> String {
        "# My Existing Project\n\n\
         ## What This Is\n\nSentinel body that must survive.\n\n\
         ## Tech Stack\n\n| Layer | Technology |\n| --- | --- |\n| Language | Rust |\n\n\
         ## Architecture\n\nSentinel architecture.\n\n\
         ## Constraints\n\n- none\n\n\
         ## Response Style\n\n- concise\n\n\
         ## Freshness\n\n- none\n\n\
         ## Project Language\n\n- `PROJECT_LANGUAGE`: `en`\n\n\
         ## Protected Paths\n\n- none\n"
            .to_string()
    }

    #[test]
    fn parse_init_repo_options_defaults_target_to_cwd() {
        let options = parse_init_repo_options(&["init-repo".to_string()]).unwrap();
        assert!(!options.blank);
        assert!(options.project_name.is_none());
    }

    #[test]
    fn scan_adopt_existing_collects_docs_and_stack() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("README.md"), "hi\n").unwrap();
        fs::create_dir_all(temp.path().join("docs").join("adr")).unwrap();
        fs::write(temp.path().join("docs").join("guide.md"), "guide\n").unwrap();
        fs::write(
            temp.path().join("docs").join("adr").join("0001.md"),
            "adr\n",
        )
        .unwrap();
        fs::write(temp.path().join("Cargo.toml"), "[package]\nname='x'\n").unwrap();

        let (docs, hints) = scan_adopt_existing(temp.path()).unwrap();
        assert!(docs
            .iter()
            .any(|doc| doc.path == "README.md" && doc.doc_type == "README"));
        assert!(docs
            .iter()
            .any(|doc| doc.path == "docs/guide.md" && doc.doc_type == "docs"));
        assert!(docs
            .iter()
            .any(|doc| doc.path == "docs/adr/0001.md" && doc.doc_type == "ADR"));
        assert!(hints.contains(&"Rust".to_string()));
    }

    #[test]
    fn existing_repo_preserves_dev_and_regenerates_adapters() {
        let temp = TempDir::new().unwrap();
        let dev_dir = temp.path().join(".dev");
        fs::create_dir_all(&dev_dir).unwrap();
        let project_sentinel = sentinel_project_md();
        let state_sentinel = "# State\n\nsentinel content that must survive\n";
        fs::write(dev_dir.join("project.md"), &project_sentinel).unwrap();
        fs::write(dev_dir.join("state.md"), state_sentinel).unwrap();

        let opts = InitRepoOptions {
            target_path: temp.path().to_path_buf(),
            project_name: None,
            blank: false,
        };
        let err = run_init_repo(&opts).expect_err("initialized repo must return Err");
        assert_eq!(
            err,
            concat!(
                "this repository is already initialized (.dev/project.md exists).\n",
                "  To regenerate AGENTS.md and CLAUDE.md from .dev/project.md, run: gal render-adapters\n",
                "  To start over from the templates, delete .dev/project.md and .dev/state.md, then run gal init again."
            )
        );

        // .dev/* must be byte-identical -- never touched.
        assert_eq!(
            fs::read_to_string(dev_dir.join("project.md")).unwrap(),
            project_sentinel
        );
        assert_eq!(
            fs::read_to_string(dev_dir.join("state.md")).unwrap(),
            state_sentinel
        );

        // Half-initialized case: only .dev/project.md present
        {
            let temp_proj = TempDir::new().unwrap();
            let dev_proj = temp_proj.path().join(".dev");
            fs::create_dir_all(&dev_proj).unwrap();
            fs::write(dev_proj.join("project.md"), &project_sentinel).unwrap();

            let opts_proj = InitRepoOptions {
                target_path: temp_proj.path().to_path_buf(),
                project_name: None,
                blank: false,
            };
            let err_proj = run_init_repo(&opts_proj)
                .expect_err("half-initialized project-only repo must return Err");
            assert_eq!(
                err_proj,
                concat!(
                    "this repository is half-initialized: .dev/project.md exists but .dev/state.md is missing.\n",
                    "  Restore .dev/state.md from version control, or delete .dev/project.md and run gal init again.\n",
                    "  gal init does not overwrite .dev/project.md."
                )
            );
            assert_eq!(
                fs::read_to_string(dev_proj.join("project.md")).unwrap(),
                project_sentinel
            );
            assert!(!dev_proj.join("state.md").exists());
        }

        // Half-initialized case: only .dev/state.md present
        {
            let temp_state = TempDir::new().unwrap();
            let dev_state = temp_state.path().join(".dev");
            fs::create_dir_all(&dev_state).unwrap();
            fs::write(dev_state.join("state.md"), state_sentinel).unwrap();

            let opts_state = InitRepoOptions {
                target_path: temp_state.path().to_path_buf(),
                project_name: None,
                blank: false,
            };
            let err_state = run_init_repo(&opts_state)
                .expect_err("half-initialized state-only repo must return Err");
            assert_eq!(
                err_state,
                concat!(
                    "this repository is half-initialized: .dev/state.md exists but .dev/project.md is missing.\n",
                    "  Restore .dev/project.md from version control, or delete .dev/state.md and run gal init again.\n",
                    "  gal init does not overwrite .dev/state.md."
                )
            );
            assert_eq!(
                fs::read_to_string(dev_state.join("state.md")).unwrap(),
                state_sentinel
            );
            assert!(!dev_state.join("project.md").exists());
        }
    }

    #[test]
    fn force_flag_is_rejected_with_a_pointer_to_the_safe_path() {
        let err = parse_init_repo_options(&["init-repo".to_string(), "--force".to_string()])
            .expect_err("--force must no longer parse");
        assert_eq!(
            err,
            "`--force` has been removed, `gal init` now refuses a repository that already holds `.dev/project.md` or `.dev/state.md`, running `gal render-adapters` regenerates the repo-local adapters, and deleting both `.dev` files then running `gal init` starts over from the templates."
        );
    }

    #[test]
    fn init_repo_propagates_adapter_report_on_both_paths() {
        let temp = TempDir::new().unwrap();
        let opts = InitRepoOptions {
            target_path: temp.path().to_path_buf(),
            project_name: None,
            blank: true,
        };
        let report = run_init_repo(&opts).expect("fresh bootstrap must succeed");
        assert!(
            !report.adapter_report.written_files.is_empty(),
            "fresh bootstrap path must propagate the renderer's write report, got {:?}",
            report.adapter_report
        );
    }

    #[test]
    fn init_template_personal_conventions_row() {
        let temp = TempDir::new().unwrap();
        let opts = InitRepoOptions {
            target_path: temp.path().to_path_buf(),
            project_name: None,
            blank: true,
        };
        run_init_repo(&opts).expect("fresh bootstrap must succeed");

        let project_md_path = temp.path().join(".dev").join("project.md");
        let project_content =
            fs::read_to_string(&project_md_path).expect(".dev/project.md must be generated");

        let has_row = project_content.lines().any(|line| {
            let trimmed = line.trim();
            trimmed.starts_with('|')
                && trimmed
                    .to_ascii_lowercase()
                    .contains("personal conventions")
                && trimmed.to_ascii_lowercase().contains("off")
        });

        assert!(has_row, "gal init emits the personal conventions off row");
    }
}
