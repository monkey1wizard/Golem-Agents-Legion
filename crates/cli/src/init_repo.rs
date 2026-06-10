use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitRepoOptions {
    pub target_path: PathBuf,
    pub project_name: Option<String>,
    pub blank: bool,
    pub force: bool,
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
}

pub fn parse_init_repo_options(args: &[String]) -> Result<InitRepoOptions, String> {
    let mut blank = false;
    let mut force = false;
    let mut positionals: Vec<String> = Vec::new();

    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "--blank" => blank = true,
            "--force" => force = true,
            unknown if unknown.starts_with("--") => {
                return Err(format!("unknown option '{unknown}'"));
            }
            _ => positionals.push(arg.clone()),
        }
    }

    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    let target_path = positionals
        .first()
        .map(PathBuf::from)
        .unwrap_or(current_dir);
    let project_name = positionals.get(1).cloned();

    Ok(InitRepoOptions {
        target_path,
        project_name,
        blank,
        force,
    })
}

pub fn run_init_repo(opts: &InitRepoOptions) -> Result<InitRepoReport, String> {
    if !opts.target_path.exists() {
        return Err(format!("Target path does not exist: {}", opts.target_path.display()));
    }

    let resolved_target = opts
        .target_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve target path {}: {e}", opts.target_path.display()))?;
    let project_name = opts
        .project_name
        .clone()
        .unwrap_or_else(|| resolved_target.file_name().unwrap_or_default().to_string_lossy().to_string());

    let repo_root = repo_root()?;
    let templates_root = repo_root.join("plugins").join("gal-core").join("templates");
    let project_template_path = templates_root.join("project.md");
    let state_template_path = templates_root.join("state.md");
    if !project_template_path.is_file() {
        return Err(format!("Missing template: {}", project_template_path.display()));
    }
    if !state_template_path.is_file() {
        return Err(format!("Missing template: {}", state_template_path.display()));
    }

    let dev_dir = resolved_target.join(".dev");
    let plans_dir = resolved_target.join("docs").join("plans");
    fs::create_dir_all(&dev_dir).map_err(|e| format!("Failed to create {}: {e}", dev_dir.display()))?;
    fs::create_dir_all(&plans_dir).map_err(|e| format!("Failed to create {}: {e}", plans_dir.display()))?;

    let project_target_path = dev_dir.join("project.md");
    let state_target_path = dev_dir.join("state.md");
    if project_target_path.exists() && !opts.force {
        return Err(format!("File already exists: {}. Use --force to overwrite.", project_target_path.display()));
    }
    if state_target_path.exists() && !opts.force {
        return Err(format!("File already exists: {}. Use --force to overwrite.", state_target_path.display()));
    }

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
        project_content = project_content.replace("[Index of source docs...]", doc_table.trim_end());
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

    let sync_options = adapters::sync_options_from_config(&base::config::GalConfig::load(), resolved_target.clone(), false)
        .map_err(|e| e.to_string())?;
    adapters::run_sync(&sync_options).map_err(|e| e.to_string())?;
    graphify_auto_init(&resolved_target)?;

    Ok(InitRepoReport {
        target_path: resolved_target,
        source_docs,
        tech_hints,
    })
}

fn repo_root() -> Result<PathBuf, String> {
    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    current_dir
        .ancestors()
        .find(|candidate| candidate.join("plugins").join("gal-core").is_dir())
        .map(Path::to_path_buf)
        .ok_or_else(|| "Could not locate the GAL repo root from the current working directory".to_string())
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
    walk_files(root, &mut files, extensions).map_err(|e| format!("Failed to scan {}: {e}", root.display()))?;
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

    let graph_version_path = repo_path.join("graphify-out").join("GAL_GRAPHIFY_VERSION.txt");
    if graph_version_path.is_file() && command_available("graphify") {
        if let Some(current_version) = get_graphify_version() {
            let stamped_version = fs::read_to_string(&graph_version_path).unwrap_or_default();
            let report_info = fs::metadata(&graph_report_path).map_err(|e| format!("Failed to stat {}: {e}", graph_report_path.display()))?;
            let stamp_info = fs::metadata(&graph_version_path).map_err(|e| format!("Failed to stat {}: {e}", graph_version_path.display()))?;
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

    #[test]
    fn parse_init_repo_options_defaults_target_to_cwd() {
        let options = parse_init_repo_options(&["init-repo".to_string()]).unwrap();
        assert!(!options.blank);
        assert!(!options.force);
        assert!(options.project_name.is_none());
    }

    #[test]
    fn scan_adopt_existing_collects_docs_and_stack() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("README.md"), "hi\n").unwrap();
        fs::create_dir_all(temp.path().join("docs").join("adr")).unwrap();
        fs::write(temp.path().join("docs").join("guide.md"), "guide\n").unwrap();
        fs::write(temp.path().join("docs").join("adr").join("0001.md"), "adr\n").unwrap();
        fs::write(temp.path().join("Cargo.toml"), "[package]\nname='x'\n").unwrap();

        let (docs, hints) = scan_adopt_existing(temp.path()).unwrap();
        assert!(docs.iter().any(|doc| doc.path == "README.md" && doc.doc_type == "README"));
        assert!(docs.iter().any(|doc| doc.path == "docs/guide.md" && doc.doc_type == "docs"));
        assert!(docs.iter().any(|doc| doc.path == "docs/adr/0001.md" && doc.doc_type == "ADR"));
        assert!(hints.contains(&"Rust".to_string()));
    }
}