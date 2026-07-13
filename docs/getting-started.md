# Getting Started with gal

`gal` is a document-driven AI workflow tool that projects one workflow interface
— the `/gal` commands, golem agents, skills, and MCP servers — into every coding
agent you use (Claude Code, GitHub Copilot, Codex, Antigravity, Gemini, opencode).
This guide takes you from install to your first completed plan in one linear pass.

> Looking to manage third-party plugins/MCP across agents? That is a separate
> product, **ccync** — not covered here.

## 1. Install

Choose the channel that fits your platform.

**`cargo install --git` (available now):**

```bash
cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli
```

GAL is not on crates.io — always use `--git`. The binary is named `gal`.

**Homebrew** _(macOS / Linux — future milestone, not yet live):_

```bash
brew install monkey1wizard/tap/gal
```

**curl** _(Linux / macOS — requires a published GitHub Release):_

```bash
curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | bash
```

**irm** _(Windows — requires a published GitHub Release; BETA):_

```powershell
irm https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.ps1 | iex
```

**winget** _(Windows — future milestone, not yet live):_

```
winget install Monkey1Wizard.GAL
```

**Prebuilt binary** _(requires a published GitHub Release):_ download from GitHub Releases and place the binary on your `PATH`.

**From the Claude / Codex marketplace (two steps):** find and install the **GAL
plugin** in the Claude Code or Codex plugin marketplace (search "gal"), or add it from the [marketplace snapshot branch](https://github.com/monkey1wizard/golem-agents-legion/tree/marketplace-snapshot), then say
**"help me install gal"**. The plugin's `install-gal` skill runs one of the
commands above (Homebrew / winget / `cargo install --git`) with your consent and
verifies `gal --version`. The plugin alone is not yet a working GAL — every command
except `install-gal` needs the `gal` binary, so this second step completes setup.

> **Platform notes**
> - **Windows**: place `gal.exe` somewhere on `PATH` (e.g. `%USERPROFILE%\bin`). PowerShell's `Get-Alias gal` can shadow the binary — call it by full path if `gal` resolves to something else.
> - **macOS / Linux**: place `gal` on `PATH` (e.g. `~/.local/bin`) and ensure it is executable (`chmod +x gal`).

Verify:

```
gal --version
gal --help
```

`gal --help` lists the workflow command surface. If the command is not found,
re-check your `PATH`.

## 2. Verify the install is healthy

```
gal doctor
```

`gal doctor` runs read-only workflow health checks and exits 0 when clean
(warnings are non-blocking).

## 3. Initialize a repository

From the root of the repo you want to work in:

```
gal init         # scaffold .dev/ + .dev/plans/ and generate agent adapters
```

`gal init` creates `.dev/project.md`, `.dev/state.md`, `.dev/plans/`, and the
per-agent adapter files (`CLAUDE.md`, `GEMINI.md`, `AGENTS.md`,
`.github/copilot-instructions.md`, `.agents/rules/gal.md`). Those adapter files
are how each coding agent picks up gal's `/gal` command contract for this repo.
Review `.dev/project.md` and fill in the summary fields.

> The `gal-config` git filter (used for machine-local config redaction) is
> optional; register it with
> `git config filter.gal-config.clean 'gal clean'` and
> `git config filter.gal-config.smudge 'gal smudge'` if your repo uses it.

## 4. Your first plan

Inside any supported coding agent, drive the workflow with `/gal`:

1. `/gal planning` — turn a request into a source plan under `.dev/plans/`.
2. `/refining-plan` — lock the task list and test plan.
3. `/plan-to-prompt` — generate the execution prompt under `.dev/plans/`.
4. `/gal pipeline` — implement → correctness gate → test → audit, task by task.
5. `/gal finalize` — land and close the completed plan.

Check where you are at any time with `/gal status`, and ask `/gal whats-next`
for the single recommended next action.

## Troubleshooting

- **`gal` not found** — the binary is not on `PATH`. Re-do step 1; on Windows,
  beware the PowerShell `Get-Alias gal` shadow and call the full path.
- **`git commit` fails with `filter 'gal clean' ... 127`** — the git filter is
  registered but `gal` is not on `PATH` for git. Put `gal` on `PATH`, or
  re-register the filter with `git config filter.gal-config.clean 'gal clean'`
  (and `.smudge 'gal smudge'`) once `PATH` is fixed.
- **An agent does not see gal** — re-run `gal init` in the repo, then
  `gal doctor`. The doctor reports stale or missing surfaces and the fix to apply.
- **`/gal` commands missing in an agent** — each agent loads gal through its own
  mechanism (Claude/Antigravity native commands, Copilot/Codex skills, Gemini
  TOML). Re-run `gal init` to regenerate the repo adapters; for Claude
  specifically, restart Claude Code to refresh its plugin cache.

## Next steps

- `docs/manual.md` — full command and workflow reference.
- `docs/contributing.md` — how to contribute to gal itself.
- `docs/architecture.md` — how gal is built.
