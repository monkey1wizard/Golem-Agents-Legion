---
type: Guide
title: Setup and Initialization
description: Install the gal binary, register the plugin with supported runtimes, and initialize and maintain repository adapters.
tags:
  - setup
  - install
  - runtime
  - init
status: stable
---

# Setup and Initialization

## Installing GAL

Select the installation method appropriate for your operating system. After installation, run `gal init` within a target repository to generate repo-local adapter files.

### Installation Methods

#### Build from Source (`cargo install --git`)

```bash
cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli
```

GAL is not distributed on crates.io, so the `--git` flag is required. Specify the `gal-cli` package name when building within the Cargo workspace. The resulting executable is named `gal`.

#### Homebrew (macOS and Linux)

```bash
brew install monkey1wizard/tap/gal
```

This is the recommended package manager route for macOS and Linux users.

#### WinGet (Windows)

```sh
winget install Monkey1Wizard.GAL
```

Community package index updates can lag behind releases. Run `winget show Monkey1Wizard.GAL` to verify version availability before installing. On Windows, see the PowerShell alias note below.

#### Direct Binary Download

Download platform archives directly from GitHub Releases using the naming pattern `gal-<version>-<platform>-<arch>[.zip|.tar.gz]`. Extract the archive and place the `gal` binary (`gal.exe` on Windows) on your system `PATH`. On macOS and Linux, mark the binary executable with `chmod +x gal`.

#### Shell Script (Linux and macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | bash
```

The installer script strictly verifies downloaded archives against SHA-256 hashes in `checksums.txt`, aborting immediately on any mismatch. It then attempts keyless cosign signature verification. If cosign is installed and reports an invalid signature, installation aborts before extracting files. If cosign is unavailable or fails to retrieve signatures, the script prints a warning and proceeds using SHA-256 verification alone. Binaries are installed to `~/.local/bin`, and source assets are stored in `~/.local/share/gal`. Set `GAL_VERSION` to install a specific release.

#### PowerShell Script (Windows)

```sh
irm https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.ps1 | iex
```

The Windows script performs the same SHA-256 and cosign checks, aborting on verification failures. It installs `gal.exe` and source assets to `%LOCALAPPDATA%\Programs\gal`. Add this directory to your user `PATH`. Set `$env:GAL_VERSION` to pin a specific release version.

**PowerShell alias collision**: PowerShell includes a built-in alias `gal` mapped to `Get-Alias`. This alias can shadow the `gal` binary when running in interactive sessions. If calling `gal` invokes `Get-Alias`, call `gal.exe` explicitly, specify its full path, or remove the alias in your PowerShell `$PROFILE`:

```powershell
Remove-Item Alias:gal -Force -ErrorAction SilentlyContinue
```

### Upgrading and Maintenance

Run `gal update` to view the currently installed version alongside platform-specific upgrade commands. GAL does not include self-updating binary routines. Upgrade using your original package manager:

```bash
cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli  # Cargo
winget upgrade Monkey1Wizard.GAL                                                   # Windows
brew upgrade gal                                                                   # macOS / Linux
```

Running `gal` without arguments displays usage help and exits with code `0`. Unrecognized subcommands exit with code `64`.

## Registering Plugins per Runtime

Plugin registration is the primary installation method for runtimes supporting Markdown-based plugin containers. Registered runtimes read directly from the canonical root at `~/.gal/plugins/gal/` rendered by `gal refresh`. Runtimes without native Markdown plugin support (such as opencode) rely on file projection fallback.

To avoid duplicate command or agent listings between plugin assets and local projections, enable the corresponding `pluginMode.<runtime>` key in `~/.gal/config/config.json` and run `gal refresh`.

### Claude Code

Claude Code loads the canonical root as a directory-backed skills plugin:

1. Create a directory junction or symlink at `~/.claude/skills/gal` pointing to `~/.gal/plugins/gal`:
   - Windows (PowerShell):
     ```powershell
     New-Item -ItemType Junction -Path "$env:USERPROFILE\.claude\skills\gal" -Target "$env:USERPROFILE\.gal\plugins\gal"
     ```
   - macOS / Linux:
     ```bash
     ln -s ~/.gal/plugins/gal ~/.claude/skills/gal
     ```
2. Run `claude plugin list` and verify that `gal@skills-dir` is loaded. Claude Code reads this junction live without local caching.
3. Enable Claude plugin mode in `~/.gal/config/config.json`:
   ```json
   {
     "pluginMode": {
       "claude": true
     }
   }
   ```
4. Run `gal refresh`. When `pluginMode.claude` is `true`, GAL stops projecting commands to `~/.claude/commands/*.md` and removes existing copies, preventing duplicate skill listings.

Installing via marketplace snapshots (`claude plugin add`) is a secondary fallback that copies assets to `~/.claude/plugins/cache/<marketplace>/gal/<version>/`.

### OpenAI Codex

Codex loads GAL through a local plugin marketplace directory:

1. Register the local marketplace directory:
   ```bash
   codex plugin marketplace add ~/.gal/plugins
   ```
   This loads `~/.gal/plugins/.agents/plugins/marketplace.json` generated by `gal refresh`.
2. Add the plugin:
   ```bash
   codex plugin add gal@gal
   ```
   Codex copies plugin files into `~/.codex/plugins/cache/gal/gal/<version>/`.
3. Run `codex plugin list` to verify that `gal@gal` appears.
4. Enable Codex plugin mode in `~/.gal/config/config.json`:
   ```json
   {
     "pluginMode": {
       "codex": true
     }
   }
   ```
5. Run `gal refresh`.

**Shared skill directory notice**: Codex and opencode share `~/.agents/skills`. If opencode is not used on the machine, setting `pluginMode.codex: true` stops projecting skills into `~/.agents/skills` and removes existing files. If both runtimes run concurrently, GAL continues projecting skills into `~/.agents/skills` to support opencode, meaning Codex displays both plugin and projected entries.

### GitHub Copilot

GitHub Copilot loads GAL as an Agent Plugins 1.0.0 package:

1. Register `~/.gal/plugins` as a directory marketplace in Copilot configuration, which references `~/.gal/plugins/.claude-plugin/marketplace.json`, then enable `gal`. Copilot reads files in place without copying. It loads agent personas from `com.github.copilot/agents/`, workspace rules from `com.github.copilot/rules/`, and MCP tools from `mcp.json`.
2. Run `copilot plugin list` to verify that `gal` is enabled.
3. Enable Copilot plugin mode in `~/.gal/config/config.json`:
   ```json
   {
     "pluginMode": {
       "copilot": true
     }
   }
   ```
4. Run `gal refresh`. When `pluginMode.copilot` is `true`, GAL stops projecting agents to `~/.copilot/agents/*.agent.md` and cleans up old copies. Copilot command skills remain in `~/.copilot/skills/` because Copilot does not parse commands from a plugin's `commands/` directory.

### Google Antigravity

Antigravity registers GAL via local directory discovery:

1. Install the plugin directly from the canonical root:
   ```bash
   agy plugin install ~/.gal/plugins/gal
   ```
   This creates a junction at `~/.gemini/antigravity-cli/plugins/gal` pointing to the canonical root.
2. Resolve duplicate `claude-code` imports if present. If Antigravity previously imported plugins from Claude Code, `agy plugin list` may show duplicate `gal` entries (`local-install` and `claude-code`). Open `~/.gemini/config/import_manifest.json`, remove the `claude-code` entry from the `imports` array, and keep only `local-install`. Run `agy plugin list` to confirm a single active entry.
3. Enable Antigravity plugin mode in `~/.gal/config/config.json`:
   ```json
   {
     "pluginMode": {
       "agy": true
     }
   }
   ```
4. Run `gal refresh`. When `pluginMode.agy` is `true`, GAL stops projecting command skills into `~/.gemini/antigravity-cli/skills/` and removes existing files.

### opencode (Projection Fallback)

opencode does not support Markdown-based plugin manifests. Its extension model relies on JavaScript and TypeScript modules installed via npm or local directories. As a result, opencode uses GAL's file-projection fallback. Running `gal refresh` projects Markdown commands to `~/.config/opencode/commands/`, agents to `~/.config/opencode/agents/`, and core skills to `~/.agents/skills/`.

### Conversational Installation

Users exploring GAL via Claude Code or Codex marketplace snapshots can complete initial installation interactively:

1. Install the **GAL plugin** from the Claude Code or Codex marketplace using the search term "gal", or load it from the repository snapshot branch.
2. In chat, instruct your assistant: **"help me install gal"**. The built-in `install-gal` skill prompts for confirmation, then calls the appropriate package manager (Homebrew, WinGet, or `cargo install --git`) to install the executable. After installation, it runs `gal init` in fresh repositories or `gal render-adapters` in existing ones.

The marketplace plugin alone does not provide a functioning environment. Workflow commands depend on the native `gal` binary. After installing the binary, configure runtime plugin registration following the steps above.

## Repository Initialization and Adapter Maintenance

### Initializing a Repository (`gal init`)

In a new repository, `gal init` creates adapter roots (`AGENTS.md` and `CLAUDE.md`) from `gal-core` templates and initializes `.dev/`, including `.dev/project.md` and `.dev/state.md`. If a repository is already initialized or partially initialized, `gal init` aborts without writing files and exits with code `1`.

| Repository State | `.dev/project.md` | `.dev/state.md` | `gal init` Action | Exit Code |
| --- | --- | --- | --- | --- |
| Uninitialized | Absent | Absent | Creates both state files from templates and renders adapter roots. | 0 |
| Initialized | Present | Present | Aborts because `.dev/project.md` exists. Directs to `gal render-adapters`. | 1 |
| Partially Initialized | Present | Absent | Aborts because `.dev/state.md` is missing. Directs to restore from git. | 1 |
| Partially Initialized | Absent | Present | Aborts because `.dev/project.md` is missing. Directs to restore from git. | 1 |

Error messages provide actionable remediation steps:

```text
gal init: this repository is already initialized (.dev/project.md exists).
  To regenerate AGENTS.md and CLAUDE.md from .dev/project.md, run: gal render-adapters
  To start over from the templates, delete .dev/project.md and .dev/state.md, then run gal init again.
```

```text
gal init: this repository is half-initialized: .dev/project.md exists but .dev/state.md is missing.
  Restore .dev/state.md from version control, or delete .dev/project.md and run gal init again.
  gal init does not overwrite .dev/project.md.
```

### Regenerating Adapters (`gal render-adapters`)

In an initialized repository, `gal render-adapters` updates adapter files using `.dev/project.md` as the single authoritative source.

| Repository State | `.dev/project.md` | `gal render-adapters` Action | Exit Code |
| --- | --- | --- | --- |
| Initialized | Present | Regenerates `AGENTS.md`, `CLAUDE.md`, and conditional layers, pruning retired roots. | 0 |
| Uninitialized | Absent | Aborts: `gal render-adapters: .dev/project.md not found. Run gal init first.` | 1 |

The `gal render-adapters` command does not inspect `.dev/state.md`.

### Adapter Cleanup and Retired Files

When updating existing repositories or migrating from versions that produced five adapter roots, `gal render-adapters` cleans up deprecated artifacts:

- **Retired root removal**: Former bridge files (`GEMINI.md`, `.github/copilot-instructions.md`, and `.agents/rules/gal.md`) are retired. When refreshing adapters, GAL inspects these paths. If the initial line contains a GAL generation marker, the file is deleted and reported as `pruned (GAL-owned)`. If removing files leaves `.agents/rules/` or `.github/` empty, the containing directory is also removed.
- **Preserving user files**: Files lacking a GAL generation marker are treated as user files. GAL leaves them untouched and reports them as `kept (hand-owned)`.
- **Conditional layers**: When Rust conventions are enabled in `.dev/project.md`, conditional rule files (`.claude/rules/gal-rust.md` and `.github/instructions/gal-rust.instructions.md`) are updated alongside main adapters. If Rust conventions are disabled, stale GAL-managed rule files are removed.

### Project Specification (`.dev/project.md`)

The `.dev/project.md` file defines the core project profile used for adapter rendering. The file **must contain exactly eight H2 sections once**: `What This Is`, `Tech Stack`, `Architecture`, `Constraints`, `Response Style`, `Freshness`, `Project Language`, and `Protected Paths`.

Validation fails closed. Because `gal init` provisions this file from templates, checks guard against structural corruption from manual edits. If a section is absent or duplicated, `gal render-adapters` aborts execution, reports the problematic heading, and writes no files. Only one error is reported per run. If multiple headings are missing, resolve them against `plugins/gal-core/templates/project.md` and rerun the command.

Size limits (`PROJECT_MD_MAX_BYTES`) are strictly enforced. If a file exceeds the byte limit, trim content to proceed. The generator never emits partial adapter files to fit within byte limits.

Below the `Tech Stack` table, `.dev/project.md` includes an authoritative check block:

```markdown
<!-- gal:authoritative-check -->
```json
{
  "command": [
    "cargo test --workspace",
    "cargo clippy --workspace",
    "cargo fmt --check"
  ]
}
```
```

Commands in this array run sequentially without shell variable interpolation. Spawned processes execute in the current working directory of the caller without changing directory to repository root. In documentation repositories without test suites, set the array to `["true"]`. The `gal finalize-check` gate executes these commands during pipeline verification.

## Running Diagnostics (`gal doctor`)

Run `gal doctor` to verify system health across the binary, canonical roots, runtime configurations, and machine settings. Execute this check after initial setup, following upgrades, or whenever an agent fails to recognize commands.

Key diagnostic checks include:

- **opencode projection sync**: Flags command and agent files in `~/.config/opencode/` that diverge from the canonical root. Outdated files generate warnings, while missing files generate errors. Run `gal refresh` to resolve discrepancies.
- **Claude skills junction**: Warns if `~/.claude/skills/gal` is missing and prints manual creation steps. The `gal refresh` command does not create this link automatically.
- **Adapter size analysis**: In initialized repositories, reports byte counts for `AGENTS.md` and `CLAUDE.md`. Files exceeding recommended sizes produce informational `[WARNING]` notices without interrupting operation.

Running `gal doctor` alone does not invoke headless coding CLIs. To test headless execution, use `gal doctor --executor-smoke`. This executes mock tasks through the headless dispatch pipeline, testing CLI installation, login status, and receipt generation across `codex`, `claude`, `copilot`, `agy`, and `opencode`. Add `--transport ssh` to test remote execution lanes, or `--json --strict` for CI environments. For routing and model configuration, see [configuration.md](configuration.md#executor-routing-executorrouting). For execution workflows, see [workflows.md](workflows.md#task-execution-cycle-and-roles).

## Runtime Invocation Syntax and Caching

### Command Syntax per Runtime

Because runtimes register capabilities differently, trigger syntax varies across tools:

| Runtime | Command Syntax | Notes |
| --- | --- | --- |
| Claude Code | `/gal status` | Native plugin command. |
| OpenAI Codex | `/gal-status` | Registered as a skill. The `$` prefix (`$gal-status`) and `/skills` syntax are also supported. |
| GitHub Copilot | `/gal-status` | Registered as a skill, visible via `/skills list`. |
| Google Antigravity | `/gal-status` | Registered as an agent skill. |
| opencode | `/gal-status` | Native Markdown command. |

Note syntax differences: Claude Code uses space-separated commands (`/gal status`), while other runtimes use hyphenated skill triggers (`/gal-status` or `$gal-status`).

### Update Latency and Caching

The time required for command updates to take effect depends on runtime caching behavior:

- **Claude Code**: Reads canonical files dynamically through the `gal@skills-dir` junction. Changes take effect on the next conversational turn. Marketplace installations require a client restart.
- **Google Antigravity**: Loads commands during process startup. Changes require restarting `agy`.
- **GitHub Copilot and opencode**: Read command and skill definitions at the start of each new conversation session.
- **OpenAI Codex**: Automatically detects skill updates in active threads. If changes do not reflect, restart Codex or open a new conversation.

**Deprecated runtimes**: GAL does not project commands or skills to Gemini CLI. Former Gemini CLI users should transition to Google Antigravity, which mounts skills at `~/.gemini/antigravity-cli/skills/<name>/SKILL.md`.

Codex runtime considerations:

- **Context budget truncation**: Codex caps initial skill list sizes. Under tight context budgets, it shortens descriptions or omits skills from autocomplete menus. Omitted skills can still be executed using `$skill-name`.
- **Duplicate skill listings**: If multiple tools register skills with identical names, Codex lists both entries in its selection menu.
