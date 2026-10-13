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

After you register a plugin, run `gal refresh`. GAL detects local registration evidence and projects fallback files when registration is absent or uncertain. An uncertain result produces a warning and keeps fallback files available.

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
3. Run `gal refresh`. GAL detects the Claude plugin registration and omits command fallback files only when registration is confirmed. Otherwise, it projects fallback commands and reports uncertain registration evidence.

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
4. Run `gal refresh`. Confirmed Codex registration owns core skills. GAL continues to project TOML agents and commands.

Codex pipeline hooks are included in the Codex plugin manifest at `.codex-plugin/plugin.json#hooks`. GAL does not install root default hooks or hooks in the Claude Code manifest. After refreshing, verify the Codex plugin manifest and trust the GAL plugin in Codex before relying on guarded continuation. Trust and hook execution are runtime facts. `gal doctor` can report static packaging and command readiness, but only the live same-session handshake can prove that Codex ran a trusted hook.

**Shared skill directory notice**: Codex and opencode share `~/.agents/skills`. GAL keeps this shared surface projected whenever opencode is selected, even when Codex registration is confirmed. Codex registration does not remove files needed by opencode.

## Codex Pipeline Execution Setup

This procedure covers Codex CLI 0.161.0 and Codex Desktop 26.1002.7124.0 on Windows, the versions observed on 2026-10-08. If you use a different version, verify current behavior in the official Codex documentation. Official setting names and permission boundaries are documented in [permission modes](https://learn.chatgpt.com/docs/permission-modes), [agent approvals and security](https://learn.chatgpt.com/docs/agent-approvals-security), and the [configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference).

### Known issue: the Codex Windows sandbox fails

On Windows, the Codex sandbox currently fails before it runs any command. A GAL pipeline cannot run inside it. This is a defect in Codex, not in GAL or in the repository, and GAL cannot repair it.

**Symptoms.** Every sandboxed command fails with `helper_unknown_error: setup refresh had errors`. The agent then reports that it cannot read files or start a shell. A pipeline phase ends with Verdict `BLOCKED`, `no-receipt`, or zero tests run, even though the executor itself started.

**How to confirm.** Open `%USERPROFILE%\.codex\.sandbox\sandbox.<date>.log` and search for `setup refresh completed with errors`. The reported cause varies between runs. Observed causes include a Codex runtime file that is in use (os error 32), a failed deny ACE, and a failed write ACE. Restarting Codex, restarting the sandbox service, and rebooting did not clear the failure. To test a machine directly, run this command in a disposable directory:

```powershell
codex exec --json -s workspace-write "Run the command Get-Location in PowerShell and report its output"
```

The machine is affected when the `command_execution` event reports `exit_code` `-1` with `setup refresh had errors`.

**What GAL does.** GAL launches every dispatched Codex child with `--dangerously-bypass-approvals-and-sandbox`. The child therefore runs without the Codex sandbox, at the same full-access trust level that the other executors receive from their permission-bypass flags. Earlier GAL versions launched the child with `-s workspace-write`. Under this defect, that setting made every Codex dispatch fail.

**What you must do.** Run the outer Codex session that drives a GAL pipeline without the sandbox. See [Required permission mode: Full access](#required-permission-mode-full-access). **Approve for me** (auto-review) is not a workaround. It only chooses who reviews approval requests, and its commands still run inside the failing sandbox.

**Security impact.** Without the sandbox, Codex can read and write the whole filesystem and run any terminal command available to your account. Run GAL pipelines only on trusted development machines and in trusted repositories, as the [permission-bypass warning](workflows.md#security-warning-permission-bypass) already requires for every executor.

**When Codex is fixed.** Re-run the test command above. Restoring a sandboxed outer session then becomes possible again. GAL keeps launching its Codex child without the sandbox until a GAL release states otherwise.

### Shared prerequisites

1. Install GAL and verify a complete Codex CLI installation. In the shell session that will execute dispatch, run:

   ```powershell
   gal.exe --version
   codex --version
   codex doctor
   Get-Command gal.exe -All
   Get-Command codex -All
   ```

   In other shells, verify the resolved executable using that shell's command lookup tool. On Windows, do not modify the system `PATH` to address lookup failures. If PowerShell resolves `gal` to its built-in alias, invoke `gal.exe` directly or specify its full path.

2. Complete Codex authentication separately using `codex login`. Do not expose or inspect token files directly. Verify all executors configured for the planned task, not just Codex.
3. Open your trusted target workspace. Restrict network access to required destinations. Only add writable directories when diagnostics identify the specific path needed.
4. Run `gal doctor`, then execute `gal doctor --executor-smoke --executor codex` in a disposable test directory. Note that the smoke test consumes provider quota. It verifies CLI readiness and receipt handling, but does not prove that a full pipeline, independent audit, or Desktop permission mode will succeed.
5. Configure `executorRouting` and `timeoutSecs` independently from Codex approval settings. See [configuration.md](configuration.md#executor-routing-executorrouting).
6. Initialize only a new repository. When multiple plans are active, explicitly specify the intended prompt.

### Codex guarded continuation readiness

The guarded `codex_stop_v1` profile requires a trusted GAL Codex plugin and a live same-session hook handshake. No model name, environment guess, stale marker, or missing marker for another host enables this profile. An ordinary `gal pipeline <prompt>` call without a valid grant stays on `legacy_interactive`. A host-neutral ordinary invocation cannot detect an absent, disabled, or untrusted Codex hook when the hook never runs.

The hook accepts an explicit execution-prompt path, a `#file:` or `@` source-plan reference that resolves to its paired execution prompt, or a pathless request when `.dev/state.md` contains exactly one active plan. It rejects missing or ambiguous active-plan state, paths outside `.dev/plans/`, unsafe path syntax, and `from` or `stop-at` modifiers. Guarded continuation does not silently discard those modifiers. Use the typed task and phase action returned by the guarded coordinator.

Before relying on guarded continuation, complete these checks:

1. Run `gal refresh` after registering the Codex plugin. GAL detects registration automatically.
2. Confirm the installed GAL Codex plugin manifest contains GAL's `UserPromptSubmit`, `PreToolUse`, and `Stop` hooks. Do not add them to root `hooks/hooks.json`, root `plugin.json`, or the Claude Code manifest.
3. Run `gal doctor` to check static packaging and command readiness. Trust the GAL plugin in Codex. Static diagnostics cannot prove runtime trust or hook execution.
4. Start an unambiguous pipeline request in the trusted target workspace. Allow Codex to run the same-session Stop canary. The canary must acknowledge bootstrap and return the exact grant-bearing continuation. The next `UserPromptSubmit` must bind that continuation in the same session before the guarded CLI entry consumes the grant. The guarded entry then runs its fresh entry latch and semantic task checkpoint before implementation can start.
5. If the hook is absent, disabled, untrusted, or the canary fails, do not rely on guarded continuation. An explicit `--require-codex-stop-v1` entry must fail before implementation with `host-continuation-not-ready` when its grant is missing or invalid. An ordinary invocation stays on `legacy_interactive`; the host-neutral CLI cannot infer that it is running in Codex.

The Codex `PreToolUse` hook denies every supported tool call while bootstrap is pending. After the Stop canary creates a one-time grant, it permits only the exact grant-bearing first action in the same session. Unsupported specialized or hosted tool paths are outside this guarded bootstrap guarantee and cannot be used by the bootstrap continuation. After guarded entry consumes the grant and creates coordinator revision 0, this bootstrap restriction ends. The active hook returns neutral/pass for normal tool use. This result does not authorize a checkpoint or phase transition. GAL's guarded actions still enforce coordinator revisions, receipts, and task-quality checks. The Stop hook blocks premature final responses until a terminal or typed human-required state.

If guarded continuation stops, keep the workspace and session identity unchanged while diagnosing it. Run `gal doctor` for static readiness, then inspect the coordinator and attempt evidence under `.dev/pipeline/<plan-scope>/`. Resolve a trust, manifest, or handler problem and run `gal refresh` if projected assets are stale. Start a new trusted same-session handshake to obtain a fresh grant. Do not copy, hand-author, or reuse a grant. If identity or evidence checks report a conflict, preserve the recorded state and resolve that conflict before retrying. To continue without guarded Codex behavior, use the ordinary `gal pipeline <prompt>` path and its existing host-owned continuation flow.

If the executable is missing from the dispatch shell context, readiness reports **Missing**. If command lookup is explicitly blocked by permissions or security policy, it reports **Denied**. Incomplete or conflicting results report **Unknown**. A failed lookup alone does not establish that Codex is uninstalled, do not attempt a reinstallation based solely on a denied or unknown lookup.

### Required permission mode: Full access

Use this mode for every outer Codex session that runs a GAL pipeline while the [Windows sandbox defect](#known-issue-the-codex-windows-sandbox-fails) is present. Earlier versions of this guide recommended **Approve for me** with RTK and GAL command rules, or a custom `workspace-write` configuration. Both options run commands inside the failing sandbox, so neither one can run a pipeline today.

1. In Codex Desktop, select the **Full access** permission mode for the task.
2. For the Codex CLI, start the session with `--dangerously-bypass-approvals-and-sandbox`, or set the sandbox mode in `%USERPROFILE%\.codex\config.toml`. Back up the file first. Preserve unrelated settings, and place the key before any table:

   ```toml
   sandbox_mode = "danger-full-access"
   ```

   An approval setting such as `approvals_reviewer = "auto_review"` can stay. It does not bring the sandbox back.
3. Restart Codex so that it reloads `config.toml`.
4. Before running a pipeline, confirm the execution environment:

   ```powershell
   gal.exe --version
   codex login status
   gal.exe doctor --executor-smoke --executor codex --timeout 90
   ```

   The doctor run must start Codex and reach a terminal provider result without TLS, sandbox, or approval errors. Receipt validation is a separate check. If the doctor reports `NO_RECEIPT` or another receipt error, inspect the generated receipt and attempt log.

GAL never changes Codex approval or sandbox settings. The outer session mode stays your decision. The dispatched Codex child runs without the sandbox regardless of the outer mode. Organization-managed restrictions still apply to the outer session. Refer to official OpenAI documentation for [sandbox and approval behavior](https://developers.openai.com/codex/sandboxing).

If this setup misses an environment-specific case or you encounter a related problem, please [open an issue](https://github.com/monkey1wizard/Golem-Agents-Legion/issues/new).

### Source worktree runtime and release boundary

Each source worktree keeps its pipeline build cache, immutable runtime generations, and coordinator state under its own canonical `target/gal-pipeline/` directory. These private roots isolate source worktrees from each other and from the installed GAL used by downstream repositories.

Before release, validate package-manager artifacts in an isolated fixture: install the candidate artifact into the fixture, verify its version and runtime behavior there, and keep the fixture separate from the user's shared installation. Source worktrees do not promote or replace the installed shim. The shared shim changes only through a package-manager release. Source updates use private immutable generations and safe rebinds. Continue using the installed GAL for downstream repositories.

### Smoke testing, pipeline validation, and rollback

Use a disposable repository and a minimal plan that creates a single file to test your configuration. Execute the targeted Codex smoke test, then run a pipeline exercising implementation, testing, independent audit, and continuation after an interrupted phase. Inspect the attempt log, execution receipt, and final gate evidence. Each stage provides distinct evidence: a successful command lookup confirms binary availability, a process launch confirms execution started, and a provider response validates API communication. Valid execution receipts and passing gate checks are evaluated separately.

If access is denied, retain the pipeline recovery point and request native Codex approval only for the specific bounded operation, provided your active permission mode supports it. If the cause of a failure is unknown, preserve diagnostic logs and investigate the observed stage directly rather than assuming an upstream model or authentication defect without supporting data. Never disable TLS certificate validation, grant permissions across an entire user home directory, or alter execution routing during an active attempt.

To roll back configuration changes, restore only the modified settings from your backup, reselect the previous Codex Desktop mode, and reconfirm the effective task context. Do not replace the entire configuration file or remove user credentials.

### Developer acceptance checklist

Record results independently of automated pipeline output and finalization gates. Leave manual and live environment checks marked as **NOT RUN** until a developer executes them.

- [ ] Record Codex Desktop and CLI versions alongside the selected permission mode.
- [ ] Verify user session login without exposing credential data or token files.
- [ ] Verify each routed executor and run the executor smoke test in a disposable directory.
- [ ] Run a minimal pipeline and inspect implementation, test, independent audit, and continuation evidence.
- [ ] Verify error handling and recovery behavior for denied and unknown states in your environment.
- [ ] Document rollback settings and confirm the previous configuration mode can be restored.

### GitHub Copilot

GitHub Copilot loads GAL as an Agent Plugins 1.0.0 package:

1. Register `~/.gal/plugins` as a directory marketplace in Copilot configuration, which references `~/.gal/plugins/.claude-plugin/marketplace.json`, then enable `gal`. Copilot reads files in place without copying. It loads agent personas from `com.github.copilot/agents/`, workspace rules from `com.github.copilot/rules/`, and MCP tools from `mcp.json`.
2. Run `copilot plugin list` to verify that `gal` is enabled.
3. Run `gal refresh`. Confirmed Copilot registration owns agents. GAL continues to project command skills because Copilot does not parse commands from a plugin's `commands/` directory.

### Google Antigravity

Antigravity registers GAL via local directory discovery:

1. Install the plugin directly from the canonical root:
   ```bash
   agy plugin install ~/.gal/plugins/gal
   ```
   This creates a junction at `~/.gemini/antigravity-cli/plugins/gal` pointing to the canonical root.
2. Resolve duplicate `claude-code` imports if present. If Antigravity previously imported plugins from Claude Code, `agy plugin list` may show duplicate `gal` entries (`local-install` and `claude-code`). Open `~/.gemini/config/import_manifest.json`, remove the `claude-code` entry from the `imports` array, and keep only `local-install`. Run `agy plugin list` to confirm a single active entry.
3. Run `gal refresh`. Confirmed Antigravity registration owns command skills. GAL keeps the canonical plugin link available.

### opencode (Projection Fallback)

opencode does not support Markdown-based plugin manifests. Its extension model relies on JavaScript and TypeScript modules installed via npm or local directories. As a result, opencode uses GAL's file-projection fallback. Running `gal refresh` projects Markdown commands to `~/.config/opencode/commands/`, agents to `~/.config/opencode/agents/`, and core skills to `~/.agents/skills/`.

### Conversational Installation

Users exploring GAL via Claude Code or Codex marketplace snapshots can complete initial installation interactively:

1. Install the **GAL plugin** from the Claude Code or Codex marketplace using the search term "gal", or load it from the repository snapshot branch.
2. In chat, instruct your assistant: **"help me install gal"**. The built-in `install-gal` skill prompts for confirmation, then calls the appropriate package manager (Homebrew, WinGet, or `cargo install --git`) to install the executable. After installation, it runs `gal init` in fresh repositories or `gal render-adapters` in existing ones.

The marketplace plugin alone does not provide a functioning environment. Workflow commands depend on the native `gal` binary. After installing the binary, configure runtime plugin registration following the steps above.

## Repository Initialization and Adapter Maintenance

### Initializing a Repository (`gal init`)

In a new repository, `gal init` creates adapter roots (`AGENTS.md`) from `gal-core` templates and initializes `.dev/`, including `.dev/project.md` and `.dev/state.md`. If a repository is already initialized or partially initialized, `gal init` aborts without writing files and exits with code `1`.

GAL provides `AGENTS.md` as its only repository instruction root. Depending on your coding agent's instruction-loading design, an existing `CLAUDE.md` or similar file may cause the agent to skip GAL's `AGENTS.md`.

Check your coding agent's documentation for supported instruction filenames, file discovery, and loading precedence. If the agent does not load `AGENTS.md`, configure it using the method described in its documentation. Do not assume that generating the file makes every coding agent load it automatically.

| Repository State | `.dev/project.md` | `.dev/state.md` | `gal init` Action | Exit Code |
| --- | --- | --- | --- | --- |
| Uninitialized | Absent | Absent | Creates both state files from templates and renders adapter roots. | 0 |
| Initialized | Present | Present | Aborts because `.dev/project.md` exists. Directs to `gal render-adapters`. | 1 |
| Partially Initialized | Present | Absent | Aborts because `.dev/state.md` is missing. Directs to restore from git. | 1 |
| Partially Initialized | Absent | Present | Aborts because `.dev/project.md` is missing. Directs to restore from git. | 1 |

Error messages provide actionable remediation steps:

```text
gal init: this repository is already initialized (.dev/project.md exists).
  To regenerate AGENTS.md from .dev/project.md, run: gal render-adapters
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
| Initialized | Present | Regenerates `AGENTS.md`, and conditional layers, pruning retired roots. | 0 |
| Uninitialized | Absent | Aborts: `gal render-adapters: .dev/project.md not found. Run gal init first.` | 1 |

The `gal render-adapters` command does not inspect `.dev/state.md`.

### Adapter Cleanup and Retired Files

When updating existing repositories or migrating from versions that produced five adapter roots, `gal render-adapters` cleans up deprecated artifacts:

- **Retired root removal**: Former bridge files (`CLAUDE.md`, `GEMINI.md`, `.github/copilot-instructions.md`, and `.agents/rules/gal.md`) are retired. When refreshing adapters, GAL inspects these paths. If the initial line contains a GAL generation marker, the file is deleted and reported as `pruned (GAL-owned)`. If removing files leaves `.agents/rules/` or `.github/` empty, the containing directory is also removed.
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
- **Adapter size analysis**: In initialized repositories, reports byte counts for `AGENTS.md`. Files exceeding recommended sizes produce informational `[WARNING]` notices without interrupting operation.

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
