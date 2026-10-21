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

Setting up GAL on a machine takes four steps:

1. Install the `gal` binary with one of the methods below.
2. Run `gal refresh` once. It renders the canonical plugin root at `~/.gal/plugins/gal/` and projects GAL files onto your coding agents. Every runtime reads GAL from this root or from its projections, so this step must run before the runtimes can use GAL.
3. Register the plugin with each runtime you use, as described in [Registering Plugins per Runtime](#registering-plugins-per-runtime). Then run `gal refresh` again so that GAL detects the registrations.
4. Run `gal init` in each repository that will use GAL. See [Initializing a Repository](#initializing-a-repository-gal-init).

On Windows, PowerShell resolves the bare name `gal` to a built-in alias. Use `gal.exe` there, as described in the [PowerShell alias note](#powershell-alias-collision).

### Installation Methods

#### Build from Source (`cargo install --git`)

```bash
cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli --tag v0.3.2 --locked
```

Replace `v0.3.2` with the release tag you want from GitHub Releases. Release tags keep the leading `v`. Without `--tag`, Cargo builds the current `main` branch of the public repository. The `--locked` flag builds with the dependency versions in the repository's tracked `Cargo.lock`.

GAL is not published on crates.io, so the `--git` flag is required. A crates.io crate with the same name, `gal-cli`, is an unrelated tool. Running `cargo install gal-cli` without `--git` installs that tool instead of GAL. In the GAL Cargo workspace, `gal-cli` is the package name, and the installed executable is named `gal`.

#### Homebrew (macOS and Linux)

```bash
brew install monkey1wizard/tap/gal
```

This is the recommended package manager route for macOS and Linux users.

#### WinGet (Windows)

```powershell
winget install Monkey1Wizard.GAL
```

Community package index updates can lag behind releases. Run `winget show Monkey1Wizard.GAL` to verify version availability before installing.

#### Direct Binary Download

Each GitHub release publishes these assets:

- An archive for each target, named `gal-<tag>-<platform>-<arch>.tar.gz`. Windows archives use `.zip` instead. Each archive contains the `gal` binary and the GAL source assets.
- A standalone Windows binary for each Windows architecture, named `gal-<tag>-windows-<arch>.exe`.
- `checksums.txt`, which lists the SHA-256 hash of each archive.
- `checksums.txt.sig` and `checksums.txt.pem`, the cosign signature and certificate for `checksums.txt`.
- `artifact-manifest.json`, the manifest of the release artifacts.
- `install.sh` and `install.ps1`, the install scripts.

Each release builds six targets: Windows, macOS, and Linux, each for `x64` and `arm64`. `<tag>` includes the leading `v`. For example, the x64 Windows archive for v0.3.2 is `gal-v0.3.2-windows-x64.zip`. Platform names are `windows`, `darwin`, and `linux`. Architecture names are `x64` and `arm64`.

Extract the archive and place the `gal` binary on your `PATH`. On Windows, the binary is `gal.exe`. On macOS and Linux, run `chmod +x gal` if the extracted binary is not executable. If the source assets are not next to the binary, `gal refresh` uses the copy embedded in the binary. See [Source Root Lookup](#source-root-lookup).

#### Shell Script (Linux and macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | bash
```

The script verifies the downloaded archive against the SHA-256 hashes in `checksums.txt`. A hash mismatch or a missing entry stops the installation. If `cosign` is on your `PATH`, the script also verifies the signature of `checksums.txt` with keyless cosign before it extracts any file. Any cosign verification failure stops the installation. If `cosign` is not installed, or the signature files cannot be downloaded, the script prints a warning and continues with SHA-256 verification alone.

The script installs the binary to `~/.local/bin` and the source assets to `~/.local/share/gal`. Before it copies the new source assets, it deletes the whole `~/.local/share/gal` directory, so do not keep other files there. It overwrites `~/.local/bin/gal` in place. To install a specific release, set `GAL_VERSION` to its tag, including the leading `v`. The script uses the value as the release tag in the download URL, so `0.3.2` without the `v` fails to download:

```bash
curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | GAL_VERSION=v0.3.2 bash
```

#### PowerShell Script (Windows)

```powershell
irm https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.ps1 | iex
```

The Windows script is in beta. It has not yet been validated on a clean machine, and it prints a beta warning when it runs. It performs the same SHA-256 and cosign checks as the shell script and stops on the same verification failures.

The script installs `gal.exe` and the source assets to `%LOCALAPPDATA%\Programs\gal`. Before it copies the new files, it deletes everything that is already in that directory. Do not keep other files there. The script does not change your `PATH`. Add `%LOCALAPPDATA%\Programs\gal` to your user `PATH` yourself. To install a specific release, set `$env:GAL_VERSION` to its tag, including the leading `v`, for example `$env:GAL_VERSION = 'v0.3.2'`.

#### PowerShell Alias Collision

PowerShell has a built-in alias named `gal` that maps to `Get-Alias`. In every PowerShell session, including scripts, the bare name `gal` runs `Get-Alias` instead of the GAL binary. Call `gal.exe` explicitly, use the full path to the binary, or remove the alias in your PowerShell `$PROFILE`:

```powershell
Remove-Item Alias:gal -Force -ErrorAction SilentlyContinue
```

### Source Root Lookup

`gal refresh` needs a GAL source root that contains `skills/`, `agents/`, and `commands/`. It looks for one in this order:

1. The directory that contains the `gal` binary. It checks the flat layout first, with the source beside the binary. Then it checks `share/gal` beside `bin/gal` under the same prefix. Then it checks each parent directory of the binary.
2. The source payload embedded in the binary. GAL writes this copy to `~/.gal/embedded-src` and uses it.

`gal dev-apply` deploys a GAL source checkout. It accepts these options:

- `--source <path>` sets the source root. Without it, `gal dev-apply` uses the checkout found from the current directory. If no checkout is found, the command stops with an error.
- `--root <path>` limits writes to an existing root directory. It requires `--source`.

Use `gal dev-apply` only for a GAL source checkout. For machine setup, use `gal refresh`.

### Upgrading and Maintenance

GAL does not update itself. `gal update` prints the installed version and three upgrade commands, for Cargo, WinGet, and Homebrew. Upgrade with the same method that you used to install:

| Install method | Upgrade |
| --- | --- |
| Cargo | Run the same `cargo install --git ... gal-cli --tag <new tag> --locked` command with the new tag. Cargo replaces the installed binary. Add `--force` to reinstall the same revision. |
| Homebrew | `brew upgrade gal` |
| WinGet | `winget upgrade Monkey1Wizard.GAL` |
| Shell or PowerShell script | Run the install script again. |
| Direct download | Download the new release asset and replace the binary on your `PATH`. |

After every upgrade, complete these steps:

1. Run `gal refresh` to render the new canonical root and projections.
2. Run `gal render-adapters` in each initialized repository to bring its adapter files up to the new version.
3. Reload the runtimes that cache GAL, as described in [Update Latency and Caching](#update-latency-and-caching).

Running `gal` without arguments displays usage help and exits with code `0`. Unrecognized subcommands exit with code `64`.

## Registering Plugins per Runtime

Plugin registration is the primary way to connect GAL to a runtime that supports Markdown-based plugin containers. Claude Code, GitHub Copilot, and Google Antigravity read the plugin directly from the canonical root at `~/.gal/plugins/gal/`, which `gal refresh` renders. OpenAI Codex copies the plugin into its own cache when you install it. opencode has no Markdown plugin support and uses file projection instead.

After you register a plugin, run `gal refresh`. GAL looks for local registration evidence for each runtime. When registration is confirmed, GAL stops projecting the fallback files that the plugin replaces. When registration is absent, GAL projects the fallback files. When the evidence is uncertain, GAL projects the fallback files and prints a warning.

GAL projects only to the runtimes listed in `selectedRuntimes` in `~/.gal/install-state.json`. Valid names are `claude`, `codex`, `copilot`, `antigravity`, and `opencode`. GAL ignores other names. If the file is absent or the list has no valid name, GAL selects all five runtimes. For runtimes that are not selected, GAL removes the outputs it manages.

To add your own MCP servers, put them under a `servers` object in `~/.gal/local/mcp.json`. GAL merges them into the rendered `mcp.json`. If a personal server has the same name as a core server, the core server wins and GAL drops the personal entry.

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

2. Start a new Claude Code session, then run `claude plugin list` and verify that `gal@skills-dir` is loaded. Claude Code loads skills-dir plugins when a session starts. It reads the files through the junction, without copying them into a cache.
3. Run `gal refresh`. When GAL confirms the Claude registration, it does not project fallback commands to `~/.claude/commands/`. Otherwise, it projects the fallback commands.

`gal refresh` does not create the `~/.claude/skills/gal` link. `gal doctor` warns when the link is missing.

As a secondary option, you can install GAL from the local marketplace that `gal refresh` writes to `~/.gal/plugins/.claude-plugin/marketplace.json`:

```bash
claude plugin marketplace add ~/.gal/plugins
claude plugin install gal@gal
```

This route copies the plugin into `~/.claude/plugins/cache/<marketplace>/gal/<version>/`. The copy does not change when you run `gal refresh`, and Claude Code needs a restart to apply a plugin update.

### OpenAI Codex

Codex loads GAL through a local plugin marketplace directory:

1. Register the local marketplace directory:

   ```bash
   codex plugin marketplace add ~/.gal/plugins
   ```

   Codex loads `~/.gal/plugins/.agents/plugins/marketplace.json`, which `gal refresh` generates.
2. Add the plugin:

   ```bash
   codex plugin add gal@gal
   ```

   Codex copies the plugin files into `~/.codex/plugins/cache/gal/gal/<version>/`. `gal refresh` does not update this copy. After you upgrade GAL, remove the plugin with `codex plugin remove gal@gal`, add it again, and trust it again.
3. Run `codex plugin list` to verify that `gal@gal` appears.
4. Run `gal refresh`. GAL always projects Codex TOML agents to `~/.codex/agents/` and command skills to `~/.agents/skills/`. When Codex registration is confirmed and opencode is not selected, the plugin provides the core skills, and GAL stops projecting them.

Codex pipeline hooks are included in the Codex plugin manifest at `.codex-plugin/plugin.json#hooks`. GAL does not install root default hooks or hooks in the Claude Code manifest. After refreshing, verify the Codex plugin manifest and trust the GAL plugin in Codex before relying on guarded continuation. Trust and hook execution are runtime facts. `gal doctor` can report static packaging and command readiness, but only the live same-session handshake can prove that Codex ran a trusted hook.

**Shared skill directory notice**: Codex and opencode share `~/.agents/skills`. GAL keeps this shared surface projected whenever opencode is selected, even when Codex registration is confirmed. Codex registration does not remove files needed by opencode.

### GitHub Copilot

GitHub Copilot loads GAL as an Agent Plugins 1.0.0 package:

1. Register `~/.gal/plugins` as a directory marketplace in the Copilot configuration, then enable the `gal` plugin from that marketplace. Copilot reads the plugin files in place without copying them. It loads agent personas from `com.github.copilot/agents/` and workspace rules from `com.github.copilot/rules/`. GAL also ships a `mcp.json` file, but its server list is empty. GAL does not provide MCP servers.
2. Run `copilot plugin list` to verify that the `gal` plugin is enabled.
3. Run `gal refresh`. When Copilot registration is confirmed, the plugin provides the agents, and GAL stops projecting them. GAL still projects command skills to `~/.copilot/skills/`, because Copilot does not read commands from a plugin's `commands/` directory. GAL also keeps `~/.copilot/gal` as a directory link to `~/.gal`. Files written under `~/.copilot/gal` are written to `~/.gal`.

### Google Antigravity

Antigravity registers GAL through local directory discovery:

1. Install the plugin directly from the canonical root:

   ```bash
   agy plugin install ~/.gal/plugins/gal
   ```

2. Run `agy plugin list` and confirm that `gal` appears exactly once. If Antigravity previously imported plugins from Claude Code, the list may show duplicate `gal` entries from `local-install` and `claude-code`. In that case, open `~/.gemini/config/import_manifest.json`, remove the `claude-code` entry from the `imports` array, keep only `local-install`, and run `agy plugin list` again.
3. Run `gal refresh`. GAL treats Antigravity as registered when `~/.gemini/antigravity-cli/plugins/gal` resolves to a directory whose `.claude-plugin/plugin.json` file has the name `gal`. If that manifest cannot be read or has a different name, GAL treats the registration as uncertain, projects the command skills, and prints a warning. When registration is confirmed, the plugin provides the command skills, and GAL stops projecting them to `~/.gemini/antigravity-cli/skills/`. Whether or not Antigravity is registered, GAL keeps the `~/.gemini/antigravity-cli/gal` link pointing to `~/.gal/source`.

If `~/.gemini/antigravity-cli/plugins/gal` exists but `agy plugin list` does not show `gal`, GAL still treats Antigravity as registered and skips the command skills. Antigravity then has no GAL commands. Install the plugin with `agy plugin install` until `agy plugin list` shows `gal`.

### opencode (Projection Fallback)

opencode does not support Markdown-based plugin manifests. Its extension model relies on JavaScript and TypeScript modules installed via npm or local directories. As a result, opencode uses GAL's file-projection fallback. Running `gal refresh` projects Markdown commands to `~/.config/opencode/commands/`, agents to `~/.config/opencode/agents/`, and core skills to `~/.agents/skills/`.

### Deprecated Runtimes

GAL does not project commands or skills to Gemini CLI. Former Gemini CLI users should move to Google Antigravity.

### Conversational Installation

If the GAL plugin is already loaded in Claude Code or Codex but the `gal` binary is missing, you can ask your assistant to install the binary. Say **"help me install gal"** in chat. The built-in `install-gal` skill then works as follows:

1. It checks whether `gal --version` already works.
2. It selects a package manager for your operating system: Homebrew, WinGet, or `cargo install --git`.
3. It shows the exact install command and waits for your confirmation before it runs the command.
4. It verifies the result with `gal --version`. For a Cargo installation, it also runs `gal refresh`.
5. It tells you the remaining steps: `gal refresh`, then `gal init` in a new repository or `gal render-adapters` in an initialized repository.

The plugin alone does not provide a working environment. Workflow commands depend on the native `gal` binary. After installing the binary, register each runtime as described above.

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
5. If the hook is absent, disabled, untrusted, or the canary fails, do not rely on guarded continuation. An explicit `--require-codex-stop-v1` entry must fail before implementation with `host-continuation-not-ready` when its grant is missing or invalid. An ordinary invocation stays on `legacy_interactive`. The host-neutral CLI cannot infer that it is running in Codex.

The bootstrap has fixed time and attempt limits. A pending bootstrap expires after 120 seconds, and a grant expires 30 seconds after it is issued. The Stop hook handles at most 3 Stop events for one bootstrap.

The Codex `PreToolUse` hook denies every supported tool call while bootstrap is pending. After the Stop canary creates a one-time grant, it permits only the exact grant-bearing first action in the same session. Unsupported specialized or hosted tool paths are outside this guarded bootstrap guarantee and cannot be used by the bootstrap continuation. After the guarded entry consumes the grant and binds the coordinator to the session, this bootstrap restriction ends. The hook then allows normal tool use. This result does not authorize a checkpoint or phase transition. GAL's guarded actions still enforce coordinator revisions, receipts, and task-quality checks.

The `Stop` hook acts only during bootstrap. It issues the canary and blocks the final response until the grant-bearing continuation runs. When no bootstrap is pending, the `Stop` hook lets the response finish. It does not read pipeline state, and it does not hold a running pipeline open until the pipeline reaches a terminal state.

If guarded continuation stops, keep the workspace and session identity unchanged while diagnosing it. Run `gal doctor` for static readiness, then inspect the coordinator and attempt evidence under `.dev/pipeline/<plan-scope>/`. Resolve a trust, manifest, or handler problem and run `gal refresh` if projected assets are stale. Start a new trusted same-session handshake to obtain a fresh grant. Do not copy, hand-author, or reuse a grant. If identity or evidence checks report a conflict, preserve the recorded state and resolve that conflict before retrying. To continue without guarded Codex behavior, use the ordinary `gal pipeline <prompt>` path and its existing host-owned continuation flow.

If the executable is missing from the dispatch shell context, readiness reports **Missing**. If command lookup is explicitly blocked by permissions or security policy, it reports **Denied**. Incomplete or conflicting results report **Unknown**. A failed lookup alone does not establish that Codex is uninstalled. Do not reinstall Codex based only on a denied or unknown lookup.

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

GAL never changes the approval or sandbox settings in your Codex `config.toml`. The outer session mode stays your decision. GAL does write a `sandbox_mode` value into each Codex agent file that it projects to `~/.codex/agents/<name>.toml`. The dispatched Codex child runs without the sandbox regardless of the outer mode. Organization-managed restrictions still apply to the outer session. Refer to official OpenAI documentation for [sandbox and approval behavior](https://developers.openai.com/codex/sandboxing).

If this setup misses an environment-specific case or you encounter a related problem, please [open an issue](https://github.com/monkey1wizard/Golem-Agents-Legion/issues/new).

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

In an initialized repository, `gal render-adapters` updates adapter files using `.dev/project.md` as the single authoritative source. Run it from the repository root.

| Repository State | Condition | `gal render-adapters` Action | Exit Code |
| --- | --- | --- | --- |
| Initialized | `.dev/project.md` present | Regenerates `AGENTS.md` and conditional layers, and prunes retired roots. | 0 |
| Initialized | `AGENTS.md` or a conditional layer file exists without the GAL generation marker | Aborts without writing any adapter file, to protect the user-owned file. | 1 |
| Uninitialized | `.dev/project.md` absent | Aborts: `gal render-adapters: .dev/project.md not found. Run gal init first.` | 1 |
| Any | An argument is passed | Aborts: `gal render-adapters: unknown option`. | 64 |

The `gal render-adapters` command does not inspect `.dev/state.md`.

### Adapter Cleanup and Retired Files

When updating existing repositories or migrating from versions that produced five adapter roots, `gal render-adapters` cleans up deprecated artifacts:

- **Retired root removal**: Former bridge files (`CLAUDE.md`, `GEMINI.md`, `.github/copilot-instructions.md`, and `.agents/rules/gal.md`) are retired. When refreshing adapters, GAL inspects these paths. If the initial line contains a GAL generation marker, the file is deleted and reported as `pruned (GAL-owned)`. If removing files leaves `.agents/rules/` or `.github/` empty, the containing directory is also removed.
- **Preserving user files**: Files lacking a GAL generation marker are treated as user files. GAL leaves them untouched and reports them as `kept (hand-owned)`.
- **Conditional layers**: When Rust conventions are enabled in `.dev/project.md`, conditional rule files (`.claude/rules/gal-rust.md` and `.github/instructions/gal-rust.instructions.md`) are updated alongside main adapters. If Rust conventions are disabled, stale GAL-managed rule files are removed.

GAL prints the `pruned (GAL-owned)` and `kept (hand-owned)` reports as warning lines on standard error.

### Project Specification (`.dev/project.md`)

The `.dev/project.md` file defines the core project profile used for adapter rendering. The file **must contain each of these eight required H2 sections exactly once**: `What This Is`, `Tech Stack`, `Architecture`, `Constraints`, `Response Style`, `Freshness`, `Project Language`, and `Protected Paths`. The file may contain other H2 sections. The template that `gal init` provisions includes additional sections such as `Key Decisions` and `Verified Facts`.

Validation fails closed. Because `gal init` provisions this file from templates, checks guard against structural corruption from manual edits. If a required section is absent or duplicated, `gal render-adapters` aborts execution, reports the problematic heading, and writes no files. Only one error is reported per run. If multiple headings are missing, resolve them against `plugins/gal-core/templates/project.md` and rerun the command.

Two size limits apply:

- **`.dev/project.md` budget**: When the file exceeds 30,720 bytes after line-ending normalization, `gal render-adapters` prints a warning and still renders the adapters. Trim the file to stay within the budget.
- **`AGENTS.md` limit**: When the rendered `AGENTS.md` would exceed 32,768 bytes, `gal render-adapters` aborts and writes no adapter files. The generator never emits partial adapter files to fit within this limit. Trim `.dev/project.md` until the rendered file fits.

Below the `Tech Stack` table, `.dev/project.md` includes an authoritative check block:

````markdown
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
````

The `gal finalize-check` gate runs these commands in full mode. It runs them one after another, and it runs every command even when an earlier command fails. Each failed command is reported as a failed check.

GAL does not use a shell to run these commands. It splits each command on whitespace and starts the first word as the program. Shell features such as quoting, pipes, `&&`, variable interpolation, and shell built-in commands do not work. Each program must exist as an executable on `PATH`.

`gal finalize-check` treats the current working directory as the repository root. Run it from the repository root. From a subdirectory, it cannot find `.dev/project.md` and reports the check as not run.

In documentation repositories without test suites, set the array to a single command that exists on every machine that runs the check, such as `["git --version"]`. Do not use `["true"]` on Windows. Windows has no `true` executable by default, so the check fails.

## Running Diagnostics (`gal doctor`)

Run `gal doctor` to verify system health across the binary, canonical roots, runtime configurations, and machine settings. Execute this check after initial setup, following upgrades, or whenever an agent fails to recognize commands.

Key diagnostic checks include:

- **opencode projection sync**: Flags command and agent files in `~/.config/opencode/` that diverge from the canonical root. Outdated files generate warnings, while missing files generate errors. Run `gal refresh` to resolve discrepancies.
- **Claude skills junction**: Warns if `~/.claude/skills/gal` is missing and prints manual creation steps. The `gal refresh` command does not create this link automatically.
- **Adapter size analysis**: In initialized repositories, reports byte counts for `AGENTS.md`. Files exceeding recommended sizes produce informational `[WARNING]` notices without interrupting operation.

Running `gal doctor` alone does not run tasks through the headless coding CLIs. When Codex is routed, it does run `codex exec --help` as a bounded readiness probe.

To test headless execution, use `gal doctor --executor-smoke`. The smoke test sends a minimal real task through the same dispatch path that a pipeline uses, so it calls each provider and consumes quota. It tests CLI installation, login status, and receipt generation across `codex`, `claude`, `copilot`, `agy`, and `opencode`. The smoke test uses its own built-in default models instead of your `executorRouting` models.

Use these options to adjust the smoke test:

- `--executor <name>` tests one executor.
- `--timeout <seconds>` sets the per-executor timeout. The default is 300 seconds.
- `--transport ssh` tests a remote execution lane. It also requires `--ssh-target <target>` and `--remote-workdir <dir>`.
- `--json` prints machine-readable output, and `--strict` exits with code `1` when any row does not pass. Use both in CI.

For routing and model configuration, see [configuration.md](configuration.md#executor-routing-executorrouting). For execution workflows, see [workflows.md](workflows.md#task-execution-cycle-and-roles).

## Runtime Invocation Syntax and Caching

### Command Syntax per Runtime

Because runtimes register capabilities differently, trigger syntax varies across tools:

| Runtime | Command Syntax | Notes |
| --- | --- | --- |
| Claude Code | `/gal status` or `/gal-status` | `/gal status` runs the `gal` command with the argument `status`, which routes to the status procedure. |
| OpenAI Codex | `$gal-status` or `$gal status` | Codex invokes custom skills with `$` or through `/skills`, not with `/gal`. |
| GitHub Copilot | `/gal-status` | Registered as a skill. |
| Google Antigravity | `/gal-status` | Registered as an agent skill. |
| opencode | `/gal-status` | Native Markdown command. |

### Update Latency and Caching

The time required for command updates to take effect depends on runtime caching behavior:

- **Claude Code**: Loads the `gal@skills-dir` plugin when a session starts. After `gal refresh`, start a new Claude Code session. A marketplace installation keeps a cached copy and needs a reinstall and a restart.
- **OpenAI Codex**: Reads the plugin from its own cache, which `gal refresh` does not update. After a GAL upgrade, reinstall the plugin as described in [OpenAI Codex](#openai-codex). Skills that GAL projects to `~/.agents/skills/` are read from that directory. If changes do not appear, restart Codex or open a new conversation.
- **Google Antigravity**: Loads commands during process startup. Changes require restarting `agy`.
- **GitHub Copilot and opencode**: Read command and skill definitions at the start of each new conversation session.

Codex runtime considerations:

- **Context budget truncation**: Codex caps initial skill list sizes. Under tight context budgets, it shortens descriptions or omits skills from autocomplete menus. Omitted skills can still be executed using `$skill-name`.
- **Duplicate skill listings**: If multiple tools register skills with identical names, Codex lists both entries in its selection menu.
