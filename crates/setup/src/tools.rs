//! Collaborative-tools installer — `gal setup --tools` (R-04, T-019).
//!
//! Ports `scripts/Setup-Tools.ps1` (parity baseline) / `setup-tools.sh`:
//! an interactive status/install flow for the four optional collaborative
//! tools (gstack, graphify, opencli, xmachine).
//!
//! Two seams (architect Ruling 3):
//! - prompts go through [`ToolsPrompter`] (stdin-drivable; `--check` and
//!   non-interactive runs never prompt),
//! - every external probe / install / network call goes through
//!   [`ToolExec`], so TP-16 parity is verified as status-probe
//!   classification + constructed-command parity under a mocked executor —
//!   live npm/pip/network is out of fixture scope (architect C-5).

use std::io::Write;
use std::path::{Path, PathBuf};

/// Tool readiness states (checking-contract vocabulary; spellings are
/// parity-relevant output).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolState {
    Ready,
    NeedsInit,
    NotReady,
    Unavailable,
}

impl ToolState {
    pub fn as_str(self) -> &'static str {
        match self {
            ToolState::Ready => "ready",
            ToolState::NeedsInit => "available-but-needs-init",
            ToolState::NotReady => "available-but-not-ready",
            ToolState::Unavailable => "unavailable",
        }
    }

    /// Ports `Get-CollaborationLabel`.
    pub fn collaboration_label(self) -> &'static str {
        match self {
            ToolState::Ready => "yes",
            ToolState::NeedsInit | ToolState::NotReady => "partially",
            ToolState::Unavailable => "no",
        }
    }
}

/// Ports `New-ToolStatus`.
#[derive(Debug, Clone)]
pub struct ToolStatus {
    pub name: &'static str,
    pub status: ToolState,
    pub reason: String,
    pub can_install: bool,
    pub next_step: String,
}

/// Output of a captured external command.
#[derive(Debug, Clone)]
pub struct ExecResult {
    pub exit_code: i32,
    pub output: String,
}

/// External-effect seam: probes, installs, and network. The real
/// implementation is [`SystemExec`]; tests inject a mock and assert the
/// exact commands constructed.
pub trait ToolExec {
    /// `Test-CommandAvailable` — is `name` resolvable on PATH?
    fn command_available(&self, name: &str) -> bool;
    /// Run and capture (probes: versions, `opencli doctor`).
    fn capture(&mut self, program: &str, args: &[&str]) -> ExecResult;
    /// Run streamed to the console (installs). Returns the exit code.
    fn run(&mut self, program: &str, args: &[&str]) -> i32;
    /// GET a JSON document (GitHub Releases API).
    fn http_get_json(&mut self, url: &str) -> Result<serde_json::Value, String>;
    /// Download a URL to a file.
    fn download(&mut self, url: &str, target: &Path) -> Result<(), String>;
}

/// Real executor.
pub struct SystemExec;

/// Resolve a command name to a spawnable (program, prefix-args) pair.
///
/// On Windows, npm-style shims are `.cmd`/`.ps1` files that
/// `std::process::Command` cannot spawn directly; route them through
/// `cmd /c` / `powershell -File`.
fn resolve_program(name: &str) -> Option<(String, Vec<String>)> {
    let path_var = std::env::var_os("PATH")?;
    if cfg!(windows) {
        for dir in std::env::split_paths(&path_var) {
            for ext in [".exe", ".cmd", ".bat", ".ps1"] {
                let candidate = dir.join(format!("{name}{ext}"));
                if candidate.is_file() {
                    let full = candidate.display().to_string();
                    return Some(match ext {
                        ".exe" => (full, vec![]),
                        ".ps1" => (
                            "powershell".into(),
                            vec!["-NoProfile".into(), "-ExecutionPolicy".into(), "Bypass".into(), "-File".into(), full],
                        ),
                        _ => ("cmd".into(), vec!["/c".into(), full]),
                    });
                }
            }
        }
        None
    } else {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some((candidate.display().to_string(), vec![]));
            }
        }
        None
    }
}

impl ToolExec for SystemExec {
    fn command_available(&self, name: &str) -> bool {
        resolve_program(name).is_some()
    }

    fn capture(&mut self, program: &str, args: &[&str]) -> ExecResult {
        let (resolved, prefix) = match resolve_program(program) {
            Some(pair) => pair,
            None => {
                return ExecResult {
                    exit_code: 1,
                    output: format!("{program}: command not found"),
                }
            }
        };
        let mut all_args: Vec<String> = prefix;
        all_args.extend(args.iter().map(|s| s.to_string()));
        // stdin must be null: probed tools (e.g. `opencli doctor`) may block
        // waiting for input otherwise.
        match std::process::Command::new(&resolved)
            .args(&all_args)
            .stdin(std::process::Stdio::null())
            .output()
        {
            Ok(output) => ExecResult {
                exit_code: output.status.code().unwrap_or(1),
                output: format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                )
                .trim()
                .to_string(),
            },
            Err(e) => ExecResult {
                exit_code: 1,
                output: e.to_string(),
            },
        }
    }

    fn run(&mut self, program: &str, args: &[&str]) -> i32 {
        let Some((resolved, prefix)) = resolve_program(program) else {
            eprintln!("{program}: command not found");
            return 1;
        };
        let mut all_args: Vec<String> = prefix;
        all_args.extend(args.iter().map(|s| s.to_string()));
        std::process::Command::new(&resolved)
            .args(&all_args)
            .status()
            .map(|s| s.code().unwrap_or(1))
            .unwrap_or(1)
    }

    fn http_get_json(&mut self, url: &str) -> Result<serde_json::Value, String> {
        // No HTTP client dependency in the workspace: delegate to curl,
        // which both setup scripts already assume transitively (gh/git
        // toolchain machines). Failure is reported, never fatal.
        let result = self.capture(
            "curl",
            &["-fsSL", "-H", "User-Agent: GAL-CollaborativeTools-Installer", url],
        );
        if result.exit_code != 0 {
            return Err(format!("Failed to query {url}: {}", result.output));
        }
        serde_json::from_str(&result.output).map_err(|e| format!("Failed to parse {url}: {e}"))
    }

    fn download(&mut self, url: &str, target: &Path) -> Result<(), String> {
        let target_str = target.display().to_string();
        let result = self.capture("curl", &["-fsSL", "-o", &target_str, url]);
        if result.exit_code != 0 {
            return Err(format!("Download failed: {}", result.output));
        }
        Ok(())
    }
}

/// Prompt seam for the install selection (architect C-9).
pub trait ToolsPrompter {
    /// Single candidate: `Install <name> now? [Y/n]` — true = install.
    fn confirm_install(&mut self, name: &str) -> bool;
    /// Multi-candidate: return the selected names (subset of `candidates`).
    fn select_installs(&mut self, candidates: &[ToolStatus]) -> Vec<String>;
}

/// Never installs anything (non-interactive runs).
pub struct SkipAllPrompter;

impl ToolsPrompter for SkipAllPrompter {
    fn confirm_install(&mut self, _name: &str) -> bool {
        false
    }
    fn select_installs(&mut self, _candidates: &[ToolStatus]) -> Vec<String> {
        Vec::new()
    }
}

/// Stdin-driven prompter (ports `Read-InstallSelection`).
pub struct StdinToolsPrompter;

impl ToolsPrompter for StdinToolsPrompter {
    fn confirm_install(&mut self, name: &str) -> bool {
        eprintln!("  [PROMPT] {name} is not currently ready. Install {name} now? [Y/n]");
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_err() {
            return false;
        }
        !matches!(line.trim().to_ascii_lowercase().as_str(), "n" | "no")
    }

    fn select_installs(&mut self, candidates: &[ToolStatus]) -> Vec<String> {
        eprintln!("  [PROMPT] The following tools are missing or still need install-time setup. Enter one or more numbers separated by commas, or press Enter to skip all.");
        for (index, item) in candidates.iter().enumerate() {
            eprintln!("    {}. {} - {}", index + 1, item.name, item.reason);
        }
        eprintln!("  [PROMPT] Enter selection numbers (for example: 1,3)");
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_err() {
            return Vec::new();
        }
        let mut selected = Vec::new();
        for segment in line.split(',') {
            if let Ok(number) = segment.trim().parse::<usize>() {
                if number >= 1 && number <= candidates.len() {
                    let name = candidates[number - 1].name.to_string();
                    if !selected.contains(&name) {
                        selected.push(name);
                    }
                }
            }
        }
        selected
    }
}

pub const SUPPORTED_TOOLS: &[&str] = &["gstack", "graphify", "opencli", "xmachine"];

/// Filesystem context for the probes.
pub struct ToolsEnv {
    pub user_home: PathBuf,
    pub repo_root: PathBuf,
}

impl ToolsEnv {
    fn gstack_root(&self) -> PathBuf {
        self.user_home.join(".claude").join("skills").join("gstack")
    }
    fn gstack_config(&self) -> PathBuf {
        self.user_home.join(".gstack").join("config.yaml")
    }
    fn graphify_out(&self) -> PathBuf {
        self.repo_root.join("graphify-out")
    }
    fn graphify_report(&self) -> PathBuf {
        self.graphify_out().join("GRAPH_REPORT.md")
    }
    fn xmachine_cache(&self) -> PathBuf {
        self.user_home.join(".gal").join("xmachine-nodes.json")
    }
    fn downloads_root(&self) -> PathBuf {
        self.user_home.join("Downloads")
    }
}

/// Resolve `--tool` selection (ports `Resolve-ToolSelection`).
pub fn resolve_tool_selection(raw: Option<&[String]>) -> Result<Vec<&'static str>, String> {
    let Some(raw) = raw else {
        return Ok(SUPPORTED_TOOLS.to_vec());
    };
    let mut normalized: Vec<&'static str> = Vec::new();
    for value in raw {
        for item in value.split(',') {
            let trimmed = item.trim().to_ascii_lowercase();
            if trimmed.is_empty() {
                continue;
            }
            match SUPPORTED_TOOLS.iter().find(|t| **t == trimmed) {
                Some(tool) => {
                    if !normalized.contains(tool) {
                        normalized.push(tool);
                    }
                }
                None => {
                    return Err(format!(
                        "Unsupported tool '{trimmed}'. Valid values: {}.",
                        SUPPORTED_TOOLS.join(", ")
                    ));
                }
            }
        }
    }
    if normalized.is_empty() {
        return Ok(SUPPORTED_TOOLS.to_vec());
    }
    Ok(normalized)
}

// ── status probes (port of Get-*Status) ─────────────────────────────────────

fn python_version(exec: &mut dyn ToolExec) -> Option<(String, u32, u32)> {
    for (program, prefix_args) in [("python", vec![]), ("py", vec!["-3"])] {
        if !exec.command_available(program) {
            continue;
        }
        let mut args = prefix_args.clone();
        args.extend(["-c", "import sys; print(f\"{sys.version_info[0]}.{sys.version_info[1]}\")"]);
        let result = exec.capture(program, &args);
        if result.exit_code == 0 {
            let version = result.output.lines().next().unwrap_or("").trim().to_string();
            let mut parts = version.split('.');
            if let (Some(major), Some(minor)) = (
                parts.next().and_then(|p| p.parse().ok()),
                parts.next().and_then(|p| p.parse().ok()),
            ) {
                return Some((version, major, minor));
            }
        }
    }
    None
}

pub fn gstack_status(env: &ToolsEnv, exec: &mut dyn ToolExec) -> ToolStatus {
    let mut missing: Vec<&str> = Vec::new();
    if !exec.command_available("git") {
        missing.push("Git is missing");
    }
    if !exec.command_available("bun") {
        missing.push("Bun v1.0+ is missing");
    }
    if cfg!(windows) {
        if !exec.command_available("node") {
            missing.push("Node.js is required on Windows");
        }
        if !exec.command_available("bash") {
            missing.push("Git Bash or bash is required to run ./setup on Windows");
        }
    }
    if !missing.is_empty() {
        return ToolStatus {
            name: "gstack",
            status: ToolState::Unavailable,
            reason: missing.join("; "),
            can_install: false,
            next_step: "Install the missing prerequisites, then rerun this installer.".into(),
        };
    }
    if !env.gstack_root().exists() {
        return ToolStatus {
            name: "gstack",
            status: ToolState::Unavailable,
            reason: format!("Official checkout was not found at {}", env.gstack_root().display()),
            can_install: true,
            next_step: "This installer can clone gstack and run ./setup.".into(),
        };
    }
    if !env.gstack_config().exists() {
        return ToolStatus {
            name: "gstack",
            status: ToolState::NeedsInit,
            reason: format!(
                "gstack checkout exists, but {} is missing",
                env.gstack_config().display()
            ),
            can_install: true,
            next_step: "Run the official gstack setup flow so GAL can use machine-side collaboration lanes.".into(),
        };
    }
    ToolStatus {
        name: "gstack",
        status: ToolState::Ready,
        reason: "Official checkout and ~/.gstack/config.yaml are present.".into(),
        can_install: false,
        next_step: "No action required.".into(),
    }
}

pub fn graphify_status(env: &ToolsEnv, exec: &mut dyn ToolExec) -> ToolStatus {
    if env.graphify_report().exists() {
        return ToolStatus {
            name: "graphify",
            status: ToolState::Ready,
            reason: "graphify-out/GRAPH_REPORT.md is present for this repo.".into(),
            can_install: false,
            next_step: "No action required. If the graphify CLI is unavailable, GAL can still use the existing report but cannot verify freshness automatically.".into(),
        };
    }
    let Some((version, major, minor)) = python_version(exec) else {
        return ToolStatus {
            name: "graphify",
            status: ToolState::Unavailable,
            reason: "Python 3.10+ is required but no supported Python launcher was found.".into(),
            can_install: false,
            next_step: "Install Python 3.10+ and rerun this installer.".into(),
        };
    };
    if major < 3 || (major == 3 && minor < 10) {
        return ToolStatus {
            name: "graphify",
            status: ToolState::Unavailable,
            reason: format!("Python {version} detected; graphify requires Python 3.10+."),
            can_install: false,
            next_step: "Upgrade Python to 3.10+ and rerun this installer.".into(),
        };
    }
    let cli = exec.command_available("graphify");
    let module = exec
        .capture("python", &["-m", "graphify", "--version"])
        .exit_code
        == 0;
    if !cli && !module {
        return ToolStatus {
            name: "graphify",
            status: ToolState::Unavailable,
            reason: "graphify CLI is not installed.".into(),
            can_install: true,
            next_step: "This installer can run the official graphify package install and graphify install flow.".into(),
        };
    }
    if !cli && module {
        return ToolStatus {
            name: "graphify",
            status: ToolState::NeedsInit,
            reason: "graphify is installed as a Python module, but the graphify command is not on PATH.".into(),
            can_install: false,
            next_step: "Open a new terminal or add your Python Scripts directory to PATH.".into(),
        };
    }
    if !env.graphify_out().exists() {
        return ToolStatus {
            name: "graphify",
            status: ToolState::NeedsInit,
            reason: "graphify is installed, but this repo does not have graphify artifacts yet.".into(),
            can_install: false,
            next_step: "No action is required for normal GAL flow. Run /graphify . manually only if you want graph context for this repo.".into(),
        };
    }
    ToolStatus {
        name: "graphify",
        status: ToolState::NotReady,
        reason: "graphify-out/ exists, but GRAPH_REPORT.md is missing.".into(),
        can_install: false,
        next_step: "GAL will continue without graphify. Rebuild graphify artifacts manually only if you want graph context.".into(),
    }
}

pub fn opencli_status(exec: &mut dyn ToolExec) -> ToolStatus {
    if !exec.command_available("opencli") {
        let node_major: Option<u32> = if exec.command_available("node") {
            let result = exec.capture("node", &["-p", "process.versions.node.split('.')[0]"]);
            if result.exit_code == 0 {
                result.output.lines().next().and_then(|l| l.trim().parse().ok())
            } else {
                None
            }
        } else {
            None
        };
        let Some(node_major) = node_major else {
            return ToolStatus {
                name: "opencli",
                status: ToolState::Unavailable,
                reason: "Node.js 21+ and npm are required for the official OpenCLI install path.".into(),
                can_install: false,
                next_step: "Install Node.js 21+ and npm, then rerun this installer.".into(),
            };
        };
        if node_major < 21 {
            return ToolStatus {
                name: "opencli",
                status: ToolState::Unavailable,
                reason: format!("Node.js {node_major} detected; OpenCLI requires Node.js 21+."),
                can_install: false,
                next_step: "Upgrade Node.js to 21+ and rerun this installer.".into(),
            };
        }
        if !exec.command_available("npm") {
            return ToolStatus {
                name: "opencli",
                status: ToolState::Unavailable,
                reason: "npm is required for the official OpenCLI install path.".into(),
                can_install: false,
                next_step: "Install npm and rerun this installer.".into(),
            };
        }
        return ToolStatus {
            name: "opencli",
            status: ToolState::Unavailable,
            reason: "opencli CLI is not installed.".into(),
            can_install: true,
            next_step: "This installer can run npm install -g @jackwener/opencli.".into(),
        };
    }
    let doctor = exec.capture("opencli", &["doctor"]);
    if doctor.exit_code == 0 {
        return ToolStatus {
            name: "opencli",
            status: ToolState::Ready,
            reason: "opencli doctor succeeded.".into(),
            can_install: false,
            next_step: "No action required.".into(),
        };
    }
    let mut reason = "OpenCLI CLI exists, but opencli doctor reported incomplete browser bridge or local session wiring.".to_string();
    if !doctor.output.is_empty() {
        let flattened = doctor.output.split_whitespace().collect::<Vec<_>>().join(" ");
        reason = format!("{reason} Last doctor output: {flattened}");
    }
    ToolStatus {
        name: "opencli",
        status: ToolState::NeedsInit,
        reason,
        can_install: true,
        next_step: "Complete the Browser Bridge installation in Chrome or Chromium, then rerun opencli doctor.".into(),
    }
}

/// Repo-owned assets required for xmachine (ports `Get-XmachineStatus`).
const XMACHINE_REQUIRED: &[&str] = &[
    "docs/collaborative-tools/xmachine.md",
    "scripts/Test-Xmachine.ps1",
    "scripts/Test-Xmachine.sh",
    "scripts/Invoke-XmachineRemoteTask.ps1",
    "scripts/Get-XmachineRemoteResult.ps1",
    "scripts/Start-xMachine.ps1",
    "scripts/Invoke-XmachineLocalTask.sh",
    "scripts/Get-XmachineLocalResult.sh",
    "scripts/Start-xMachine.sh",
];

pub fn xmachine_status(env: &ToolsEnv) -> ToolStatus {
    let missing: Vec<&str> = XMACHINE_REQUIRED
        .iter()
        .filter(|rel| !env.repo_root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR)).exists())
        .copied()
        .collect();
    if !missing.is_empty() {
        return ToolStatus {
            name: "xmachine",
            status: ToolState::NotReady,
            reason: format!("Repo-owned xmachine assets are missing: {}", missing.join("; ")),
            can_install: false,
            next_step: "Restore the missing xmachine scripts or docs in this repo checkout.".into(),
        };
    }
    let cache = env.xmachine_cache();
    let mut readied: Vec<String> = Vec::new();
    let mut tooling_ready: Vec<String> = Vec::new();
    if cache.exists() {
        let parsed: Option<serde_json::Value> = std::fs::read_to_string(&cache)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok());
        match parsed {
            Some(value) => {
                for node in value.get("nodes").and_then(|n| n.as_array()).unwrap_or(&Vec::new()) {
                    let id = node.get("nodeId").and_then(|v| v.as_str()).unwrap_or("");
                    match node.get("status").and_then(|v| v.as_str()) {
                        Some("readied") => readied.push(id.to_string()),
                        Some("tooling-ready") => tooling_ready.push(id.to_string()),
                        _ => {}
                    }
                }
            }
            None => {
                return ToolStatus {
                    name: "xmachine",
                    status: ToolState::NeedsInit,
                    reason: format!(
                        "Repo-owned xmachine assets are present, but the machine cache at '{}' could not be parsed.",
                        cache.display()
                    ),
                    can_install: false,
                    next_step: "Fix or remove the broken ~/.gal/xmachine-nodes.json file, then rerun scripts/Test-Xmachine.ps1 against the target work node.".into(),
                };
            }
        }
    }
    if !readied.is_empty() {
        return ToolStatus {
            name: "xmachine",
            status: ToolState::Ready,
            reason: format!(
                "Repo-owned xmachine assets are present and readied nodes are cached in ~/.gal/xmachine-nodes.json: {}",
                readied.join(", ")
            ),
            can_install: false,
            next_step: "Use a readied work node id explicitly when you want to offload a bounded task.".into(),
        };
    }
    if !tooling_ready.is_empty() {
        return ToolStatus {
            name: "xmachine",
            status: ToolState::NotReady,
            reason: format!(
                "Repo-owned xmachine assets are present. Cached work nodes exist, but none are marked readied yet: {}",
                tooling_ready.join(", ")
            ),
            can_install: false,
            next_step: "Finish the remaining readiness gates for one work node before relying on xmachine offload.".into(),
        };
    }
    ToolStatus {
        name: "xmachine",
        status: ToolState::NeedsInit,
        reason: "Repo-owned xmachine assets are present, but no work node has been verified yet.".into(),
        can_install: false,
        next_step: "Run scripts/Test-Xmachine.ps1 -WorkNode <ssh-host-alias> to verify a work node and populate ~/.gal/xmachine-nodes.json.".into(),
    }
}

pub fn tool_status(name: &str, env: &ToolsEnv, exec: &mut dyn ToolExec) -> ToolStatus {
    match name {
        "gstack" => gstack_status(env, exec),
        "graphify" => graphify_status(env, exec),
        "opencli" => opencli_status(exec),
        "xmachine" => xmachine_status(env),
        other => unreachable!("unsupported tool '{other}' passed selection validation"),
    }
}

// ── install actions ──────────────────────────────────────────────────────────

/// The exact bash command Setup-Tools runs for gstack (parity-relevant).
pub fn gstack_install_command() -> String {
    [
        "set -euo pipefail",
        "mkdir -p \"$HOME/.claude/skills\"",
        "if [ ! -d \"$HOME/.claude/skills/gstack/.git\" ]; then git clone --single-branch --depth 1 https://github.com/garrytan/gstack.git \"$HOME/.claude/skills/gstack\"; fi",
        "cd \"$HOME/.claude/skills/gstack\"",
        "./setup",
    ]
    .join("; ")
}

fn install_gstack(exec: &mut dyn ToolExec, out: &mut dyn Write) -> Result<(), String> {
    let _ = writeln!(out);
    let _ = writeln!(out, "=== Install gstack ===");
    let _ = writeln!(out, "  [INFO] Running the official gstack clone + setup flow.");
    let command = gstack_install_command();
    let code = exec.run("bash", &["-lc", &command]);
    if code != 0 {
        return Err(format!("bash command failed with exit code {code}."));
    }
    let _ = writeln!(out, "  [OK] gstack install flow completed.");
    Ok(())
}

fn install_graphify(exec: &mut dyn ToolExec, out: &mut dyn Write) -> Result<(), String> {
    let _ = writeln!(out);
    let _ = writeln!(out, "=== Install graphify ===");
    let Some((_, major, minor)) = python_version(exec) else {
        return Err("Python 3.10+ is required before graphify can be installed.".into());
    };
    if major < 3 || (major == 3 && minor < 10) {
        return Err("Python 3.10+ is required before graphify can be installed.".into());
    }
    let _ = writeln!(out, "  [INFO] Installing the official graphifyy package.");
    if exec.run("python", &["-m", "pip", "install", "graphifyy"]) != 0 {
        return Err("python -m pip install graphifyy failed.".into());
    }
    let _ = writeln!(out, "  [INFO] Running the official graphify install command.");
    let code = if exec.command_available("graphify") {
        exec.run("graphify", &["install"])
    } else {
        exec.run("python", &["-m", "graphify", "install"])
    };
    if code != 0 {
        return Err("graphify install failed.".into());
    }
    let _ = writeln!(out, "  [OK] graphify install flow completed.");
    Ok(())
}

const OPENCLI_REPO_URL: &str = "https://github.com/jackwener/OpenCLI";
const OPENCLI_RELEASES_URL: &str = "https://github.com/jackwener/OpenCLI/releases";
const OPENCLI_LATEST_API_URL: &str = "https://api.github.com/repos/jackwener/OpenCLI/releases/latest";

fn install_opencli(
    env: &ToolsEnv,
    exec: &mut dyn ToolExec,
    out: &mut dyn Write,
) -> Result<Option<PathBuf>, String> {
    let _ = writeln!(out);
    let _ = writeln!(out, "=== Install OpenCLI ===");
    if !exec.command_available("opencli") {
        let _ = writeln!(out, "  [INFO] Running the official OpenCLI npm install command.");
        if exec.run("npm", &["install", "-g", "@jackwener/opencli"]) != 0 {
            return Err("npm install -g @jackwener/opencli failed.".into());
        }
    } else {
        let _ = writeln!(out, "  [OK] opencli CLI already exists; skipping npm install.");
    }

    // Resolve and download the Browser Bridge extension zip.
    let _ = writeln!(out, "  [INFO] Resolving the latest OpenCLI Browser Bridge release asset.");
    let release = exec.http_get_json(OPENCLI_LATEST_API_URL)?;
    let asset = release
        .get("assets")
        .and_then(|a| a.as_array())
        .and_then(|assets| {
            assets.iter().find(|asset| {
                asset
                    .get("name")
                    .and_then(|n| n.as_str())
                    .map(|n| n.starts_with("opencli-extension-") && n.ends_with(".zip"))
                    .unwrap_or(false)
            })
        })
        .ok_or("Could not find an opencli-extension-*.zip asset in the latest OpenCLI release.")?;
    let name = asset.get("name").and_then(|n| n.as_str()).unwrap_or("opencli-extension.zip");
    let url = asset
        .get("browser_download_url")
        .and_then(|u| u.as_str())
        .ok_or("Release asset has no browser_download_url.")?;
    let target = env.downloads_root().join(name);
    if target.exists() {
        let _ = writeln!(out, "  [SKIP] OpenCLI Browser Bridge zip already exists: {}", target.display());
    } else {
        std::fs::create_dir_all(env.downloads_root()).map_err(|e| e.to_string())?;
        let _ = writeln!(out, "  [INFO] Downloading {url}");
        exec.download(url, &target)?;
        let _ = writeln!(out, "  [OK] Downloaded OpenCLI Browser Bridge zip to {}", target.display());
    }

    let _ = writeln!(out);
    let _ = writeln!(out, "  [INFO] OpenCLI Browser Bridge install guide (official upstream):");
    let _ = writeln!(out, "    1. Download the latest opencli-extension-v{{version}}.zip from the GitHub Releases page.");
    let _ = writeln!(out, "    2. Unzip it, open chrome://extensions, and enable Developer mode.");
    let _ = writeln!(out, "    3. Click Load unpacked and select the unzipped folder.");
    let _ = writeln!(out, "  [INFO] Downloaded file path: {}", target.display());
    let _ = writeln!(out, "  [INFO] GitHub repo: {OPENCLI_REPO_URL}");
    let _ = writeln!(out, "  [INFO] Releases page: {OPENCLI_RELEASES_URL}");
    let _ = writeln!(out, "  [INFO] After completing those steps, rerun `opencli doctor`.");
    Ok(Some(target))
}

// ── orchestration ────────────────────────────────────────────────────────────

fn write_status_report(statuses: &[ToolStatus], out: &mut dyn Write) {
    let _ = writeln!(out);
    let _ = writeln!(out, "=== Collaborative Tool Status ===");
    for status in statuses {
        let _ = writeln!(out, "  - {}: {}", status.name, status.status.as_str());
        let _ = writeln!(out, "    {}", status.reason);
    }
}

/// Run the `gal setup --tools` flow. `check` = print statuses only.
pub fn run_tools(
    tool_filter: Option<&[String]>,
    check: bool,
    env: &ToolsEnv,
    exec: &mut dyn ToolExec,
    prompter: &mut dyn ToolsPrompter,
    out: &mut dyn Write,
) -> Result<(), String> {
    let selected_tools = resolve_tool_selection(tool_filter)?;

    let statuses: Vec<ToolStatus> = selected_tools
        .iter()
        .map(|name| tool_status(name, env, exec))
        .collect();
    write_status_report(&statuses, out);

    if check {
        return Ok(());
    }

    let candidates: Vec<ToolStatus> = statuses
        .iter()
        .filter(|s| s.can_install && matches!(s.status, ToolState::Unavailable | ToolState::NeedsInit))
        .cloned()
        .collect();

    let selected_installs: Vec<String> = if candidates.is_empty() {
        let _ = writeln!(out, "  [OK] No collaborative tools need installation or installer-assisted setup.");
        Vec::new()
    } else if candidates.len() == 1 {
        if prompter.confirm_install(candidates[0].name) {
            vec![candidates[0].name.to_string()]
        } else {
            let _ = writeln!(out, "  [SKIP] {} installation skipped by user.", candidates[0].name);
            Vec::new()
        }
    } else {
        let selected = prompter.select_installs(&candidates);
        if selected.is_empty() {
            let _ = writeln!(out, "  [SKIP] No collaborative tools selected for installation.");
        }
        selected
    };

    struct Summary {
        name: &'static str,
        before: &'static str,
        action: String,
        after: ToolStatus,
        downloaded: Option<PathBuf>,
    }
    let mut summaries: Vec<Summary> = Vec::new();

    for name in &selected_tools {
        let before = tool_status(name, env, exec);
        let mut action = "no-op".to_string();
        let mut downloaded: Option<PathBuf> = None;

        if selected_installs.iter().any(|s| s == name) {
            let latest = tool_status(name, env, exec);
            if latest.status == ToolState::Ready {
                action = "skipped - already ready before install step".into();
            } else if !latest.can_install {
                action = "skipped - install not applicable for this status".into();
            } else {
                let result: Result<(), String> = match *name {
                    "gstack" => install_gstack(exec, out).map(|()| {
                        action = "installed via official clone + setup".into();
                    }),
                    "graphify" => install_graphify(exec, out).map(|()| {
                        action = "installed via official graphifyy + graphify install".into();
                    }),
                    "opencli" => install_opencli(env, exec, out).map(|path| {
                        downloaded = path;
                        action = "installed CLI and downloaded Browser Bridge zip".into();
                    }),
                    _ => Ok(()),
                };
                if let Err(message) = result {
                    action = "install failed".into();
                    let _ = writeln!(out, "  [ERROR] {name} install failed: {message}");
                }
            }
        } else if candidates.iter().any(|c| c.name == *name) {
            action = "skipped by user".into();
        }

        let after = tool_status(name, env, exec);
        summaries.push(Summary {
            name,
            before: before.status.as_str(),
            action,
            after,
            downloaded,
        });
    }

    let _ = writeln!(out);
    let _ = writeln!(out, "=== Final Summary ===");
    for item in &summaries {
        let _ = writeln!(out, "  - {}", item.name);
        let _ = writeln!(out, "    before: {}", item.before);
        let _ = writeln!(out, "    action: {}", item.action);
        let _ = writeln!(out, "    after:  {}", item.after.status.as_str());
        let _ = writeln!(out, "    GAL collaboration: {}", item.after.status.collaboration_label());
        let _ = writeln!(out, "    reason: {}", item.after.reason);
        if item.after.status != ToolState::Ready && !item.after.next_step.is_empty() {
            let _ = writeln!(out, "    next:   {}", item.after.next_step);
        }
        if item.name == "opencli" {
            if let Some(path) = &item.downloaded {
                let _ = writeln!(out, "    downloaded extension zip: {}", path.display());
                let _ = writeln!(out, "    github: {OPENCLI_REPO_URL}");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Mocked executor: declared available commands, scripted capture
    /// results, recorded run/network calls.
    struct MockExec {
        available: HashSet<String>,
        captures: Vec<(String, Vec<String>, ExecResult)>,
        runs: Vec<(String, Vec<String>)>,
        run_exit: i32,
    }

    impl MockExec {
        fn new(available: &[&str]) -> Self {
            Self {
                available: available.iter().map(|s| s.to_string()).collect(),
                captures: Vec::new(),
                runs: Vec::new(),
                run_exit: 0,
            }
        }
        fn with_capture(mut self, program: &str, args: &[&str], exit_code: i32, output: &str) -> Self {
            self.captures.push((
                program.to_string(),
                args.iter().map(|s| s.to_string()).collect(),
                ExecResult { exit_code, output: output.to_string() },
            ));
            self
        }
    }

    impl ToolExec for MockExec {
        fn command_available(&self, name: &str) -> bool {
            self.available.contains(name)
        }
        fn capture(&mut self, program: &str, args: &[&str]) -> ExecResult {
            for (p, a, result) in &self.captures {
                if p == program && a.iter().map(String::as_str).collect::<Vec<_>>() == args {
                    return result.clone();
                }
            }
            ExecResult { exit_code: 1, output: String::new() }
        }
        fn run(&mut self, program: &str, args: &[&str]) -> i32 {
            self.runs.push((program.to_string(), args.iter().map(|s| s.to_string()).collect()));
            self.run_exit
        }
        fn http_get_json(&mut self, _url: &str) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({
                "assets": [
                    {"name": "opencli-extension-v1.2.3.zip",
                     "browser_download_url": "https://example.invalid/opencli-extension-v1.2.3.zip"}
                ]
            }))
        }
        fn download(&mut self, url: &str, target: &Path) -> Result<(), String> {
            self.runs.push(("download".into(), vec![url.to_string(), target.display().to_string()]));
            std::fs::write(target, b"zip").map_err(|e| e.to_string())
        }
    }

    fn env(home: &Path, repo: &Path) -> ToolsEnv {
        ToolsEnv { user_home: home.to_path_buf(), repo_root: repo.to_path_buf() }
    }

    // ── selection ────────────────────────────────────────────────────────────

    #[test]
    fn tool_selection_defaults_normalizes_and_rejects_unknown() {
        assert_eq!(resolve_tool_selection(None).unwrap(), SUPPORTED_TOOLS.to_vec());
        let picked = resolve_tool_selection(Some(&["GStack, opencli".to_string()])).unwrap();
        assert_eq!(picked, vec!["gstack", "opencli"]);
        let err = resolve_tool_selection(Some(&["vscode".to_string()])).unwrap_err();
        assert!(err.contains("Unsupported tool 'vscode'"));
    }

    // ── status-probe classification parity (TP-16 / C-5): 4 states ──────────

    #[test]
    fn gstack_states() {
        let temp = tempfile::tempdir().unwrap();
        let e = env(temp.path(), temp.path());

        // unavailable (missing prereqs)
        let mut exec = MockExec::new(&[]);
        let s = gstack_status(&e, &mut exec);
        assert_eq!(s.status, ToolState::Unavailable);
        assert!(s.reason.contains("Git is missing"));
        assert!(!s.can_install);

        // unavailable + can_install (prereqs ok, no checkout)
        let prereqs: &[&str] = if cfg!(windows) { &["git", "bun", "node", "bash"] } else { &["git", "bun"] };
        let mut exec = MockExec::new(prereqs);
        let s = gstack_status(&e, &mut exec);
        assert_eq!(s.status, ToolState::Unavailable);
        assert!(s.can_install);

        // needs-init (checkout, no config)
        std::fs::create_dir_all(e.gstack_root()).unwrap();
        let mut exec = MockExec::new(prereqs);
        let s = gstack_status(&e, &mut exec);
        assert_eq!(s.status, ToolState::NeedsInit);
        assert!(s.can_install);

        // ready
        std::fs::create_dir_all(e.gstack_config().parent().unwrap()).unwrap();
        std::fs::write(e.gstack_config(), "x").unwrap();
        let mut exec = MockExec::new(prereqs);
        assert_eq!(gstack_status(&e, &mut exec).status, ToolState::Ready);
    }

    #[test]
    fn graphify_states() {
        let temp = tempfile::tempdir().unwrap();
        let e = env(temp.path(), temp.path());
        let py_args: &[&str] = &["-c", "import sys; print(f\"{sys.version_info[0]}.{sys.version_info[1]}\")"];

        // ready via existing report
        std::fs::create_dir_all(e.graphify_out()).unwrap();
        std::fs::write(e.graphify_report(), "r").unwrap();
        let mut exec = MockExec::new(&[]);
        assert_eq!(graphify_status(&e, &mut exec).status, ToolState::Ready);
        std::fs::remove_file(e.graphify_report()).unwrap();
        std::fs::remove_dir(e.graphify_out()).unwrap();

        // unavailable: no python
        let mut exec = MockExec::new(&[]);
        let s = graphify_status(&e, &mut exec);
        assert_eq!(s.status, ToolState::Unavailable);
        assert!(s.reason.contains("Python 3.10+"));

        // unavailable: old python
        let mut exec = MockExec::new(&["python"]).with_capture("python", py_args, 0, "3.8");
        let s = graphify_status(&e, &mut exec);
        assert_eq!(s.status, ToolState::Unavailable);
        assert!(s.reason.contains("Python 3.8 detected"));

        // unavailable + can_install: python ok, no cli/module
        let mut exec = MockExec::new(&["python"]).with_capture("python", py_args, 0, "3.12");
        let s = graphify_status(&e, &mut exec);
        assert_eq!(s.status, ToolState::Unavailable);
        assert!(s.can_install);

        // needs-init: module without CLI
        let mut exec = MockExec::new(&["python"])
            .with_capture("python", py_args, 0, "3.12")
            .with_capture("python", &["-m", "graphify", "--version"], 0, "1.0");
        let s = graphify_status(&e, &mut exec);
        assert_eq!(s.status, ToolState::NeedsInit);
        assert!(!s.can_install);

        // needs-init: CLI present, repo has no artifacts
        let mut exec = MockExec::new(&["python", "graphify"]).with_capture("python", py_args, 0, "3.12");
        assert_eq!(graphify_status(&e, &mut exec).status, ToolState::NeedsInit);

        // not-ready: graphify-out exists without report
        std::fs::create_dir_all(e.graphify_out()).unwrap();
        let mut exec = MockExec::new(&["python", "graphify"]).with_capture("python", py_args, 0, "3.12");
        assert_eq!(graphify_status(&e, &mut exec).status, ToolState::NotReady);
    }

    #[test]
    fn opencli_states() {
        // unavailable: no node
        let mut exec = MockExec::new(&[]);
        let s = opencli_status(&mut exec);
        assert_eq!(s.status, ToolState::Unavailable);
        assert!(s.reason.contains("Node.js 21+"));

        // unavailable: old node
        let mut exec = MockExec::new(&["node"])
            .with_capture("node", &["-p", "process.versions.node.split('.')[0]"], 0, "18");
        let s = opencli_status(&mut exec);
        assert_eq!(s.status, ToolState::Unavailable);
        assert!(s.reason.contains("Node.js 18 detected"));

        // unavailable + can_install
        let mut exec = MockExec::new(&["node", "npm"])
            .with_capture("node", &["-p", "process.versions.node.split('.')[0]"], 0, "22");
        let s = opencli_status(&mut exec);
        assert_eq!(s.status, ToolState::Unavailable);
        assert!(s.can_install);

        // ready: doctor exit 0
        let mut exec = MockExec::new(&["opencli"]).with_capture("opencli", &["doctor"], 0, "ok");
        assert_eq!(opencli_status(&mut exec).status, ToolState::Ready);

        // needs-init: doctor fails, output flattened into reason
        let mut exec = MockExec::new(&["opencli"]).with_capture("opencli", &["doctor"], 1, "bridge\n  missing");
        let s = opencli_status(&mut exec);
        assert_eq!(s.status, ToolState::NeedsInit);
        assert!(s.can_install);
        assert!(s.reason.contains("Last doctor output: bridge missing"));
    }

    #[test]
    fn xmachine_states() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let repo = temp.path().join("repo");
        std::fs::create_dir_all(&home).unwrap();

        // not-ready: repo assets missing
        std::fs::create_dir_all(&repo).unwrap();
        let e = env(&home, &repo);
        let s = xmachine_status(&e);
        assert_eq!(s.status, ToolState::NotReady);
        assert!(s.reason.contains("xmachine assets are missing"));

        // needs-init: assets present, no cache
        for rel in XMACHINE_REQUIRED {
            let p = repo.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "x").unwrap();
        }
        assert_eq!(xmachine_status(&e).status, ToolState::NeedsInit);

        // needs-init: broken cache
        std::fs::create_dir_all(home.join(".gal")).unwrap();
        std::fs::write(e.xmachine_cache(), "{not json").unwrap();
        let s = xmachine_status(&e);
        assert_eq!(s.status, ToolState::NeedsInit);
        assert!(s.reason.contains("could not be parsed"));

        // not-ready: tooling-ready only
        std::fs::write(
            e.xmachine_cache(),
            serde_json::json!({"nodes": [{"nodeId": "mac-mini", "status": "tooling-ready"}]}).to_string(),
        )
        .unwrap();
        let s = xmachine_status(&e);
        assert_eq!(s.status, ToolState::NotReady);
        assert!(s.reason.contains("mac-mini"));

        // ready: readied node
        std::fs::write(
            e.xmachine_cache(),
            serde_json::json!({"nodes": [{"nodeId": "mac-mini", "status": "readied"}]}).to_string(),
        )
        .unwrap();
        let s = xmachine_status(&e);
        assert_eq!(s.status, ToolState::Ready);
        assert!(s.reason.contains("mac-mini"));
    }

    // ── constructed-command parity (TP-16 / C-5) ─────────────────────────────

    #[test]
    fn gstack_install_command_matches_legacy() {
        let expected = concat!(
            "set -euo pipefail; ",
            "mkdir -p \"$HOME/.claude/skills\"; ",
            "if [ ! -d \"$HOME/.claude/skills/gstack/.git\" ]; then git clone --single-branch --depth 1 https://github.com/garrytan/gstack.git \"$HOME/.claude/skills/gstack\"; fi; ",
            "cd \"$HOME/.claude/skills/gstack\"; ",
            "./setup"
        );
        assert_eq!(gstack_install_command(), expected);
    }

    #[test]
    fn graphify_install_constructs_official_commands() {
        let py_args: &[&str] = &["-c", "import sys; print(f\"{sys.version_info[0]}.{sys.version_info[1]}\")"];
        let mut exec = MockExec::new(&["python"]).with_capture("python", py_args, 0, "3.12");
        let mut out = Vec::new();
        install_graphify(&mut exec, &mut out).unwrap();
        assert_eq!(
            exec.runs,
            vec![
                ("python".to_string(), vec!["-m".into(), "pip".into(), "install".into(), "graphifyy".into()]),
                ("python".to_string(), vec!["-m".into(), "graphify".into(), "install".into()]),
            ]
        );
    }

    #[test]
    fn opencli_install_constructs_npm_command_and_downloads_asset() {
        let temp = tempfile::tempdir().unwrap();
        let e = env(temp.path(), temp.path());
        let mut exec = MockExec::new(&["node", "npm"]);
        let mut out = Vec::new();
        let downloaded = install_opencli(&e, &mut exec, &mut out).unwrap().unwrap();
        assert_eq!(exec.runs[0], ("npm".to_string(), vec!["install".into(), "-g".into(), "@jackwener/opencli".into()]));
        assert!(downloaded.ends_with("opencli-extension-v1.2.3.zip"));
        assert!(downloaded.exists());
        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains("Load unpacked"));
    }

    // ── check + orchestration ────────────────────────────────────────────────

    #[test]
    fn check_mode_prints_status_report_only_and_never_prompts_or_installs() {
        let temp = tempfile::tempdir().unwrap();
        let e = env(temp.path(), temp.path());
        struct PanicPrompter;
        impl ToolsPrompter for PanicPrompter {
            fn confirm_install(&mut self, _: &str) -> bool {
                panic!("--check must not prompt")
            }
            fn select_installs(&mut self, _: &[ToolStatus]) -> Vec<String> {
                panic!("--check must not prompt")
            }
        }
        let mut exec = MockExec::new(&[]);
        let mut out = Vec::new();
        run_tools(None, true, &e, &mut exec, &mut PanicPrompter, &mut out).unwrap();
        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains("=== Collaborative Tool Status ==="));
        assert!(printed.contains("  - gstack: unavailable"));
        assert!(printed.contains("  - xmachine:"));
        assert!(!printed.contains("Final Summary"));
        assert!(exec.runs.is_empty(), "--check must not run anything");
    }

    #[test]
    fn skipped_installs_are_reported_in_final_summary() {
        let temp = tempfile::tempdir().unwrap();
        let e = env(temp.path(), temp.path());
        // gstack installable (prereqs ok, no checkout); everything else not a candidate.
        let prereqs: &[&str] = if cfg!(windows) { &["git", "bun", "node", "bash"] } else { &["git", "bun"] };
        let mut exec = MockExec::new(prereqs);
        let mut out = Vec::new();
        run_tools(
            Some(&["gstack".to_string()]),
            false,
            &e,
            &mut exec,
            &mut SkipAllPrompter,
            &mut out,
        )
        .unwrap();
        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains("[SKIP] gstack installation skipped by user."));
        assert!(printed.contains("=== Final Summary ==="));
        assert!(printed.contains("action: skipped by user"));
        assert!(printed.contains("GAL collaboration: no"));
        assert!(exec.runs.is_empty());
    }

    #[test]
    fn selected_install_runs_and_summarizes() {
        let temp = tempfile::tempdir().unwrap();
        let e = env(temp.path(), temp.path());
        let prereqs: &[&str] = if cfg!(windows) { &["git", "bun", "node", "bash"] } else { &["git", "bun"] };
        struct YesPrompter;
        impl ToolsPrompter for YesPrompter {
            fn confirm_install(&mut self, _: &str) -> bool {
                true
            }
            fn select_installs(&mut self, c: &[ToolStatus]) -> Vec<String> {
                c.iter().map(|s| s.name.to_string()).collect()
            }
        }
        let mut exec = MockExec::new(prereqs);
        let mut out = Vec::new();
        run_tools(
            Some(&["gstack".to_string()]),
            false,
            &e,
            &mut exec,
            &mut YesPrompter,
            &mut out,
        )
        .unwrap();
        assert_eq!(exec.runs.len(), 1);
        assert_eq!(exec.runs[0].0, "bash");
        assert_eq!(exec.runs[0].1[0], "-lc");
        assert!(exec.runs[0].1[1].contains("git clone --single-branch --depth 1"));
        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains("action: installed via official clone + setup"));
    }
}
