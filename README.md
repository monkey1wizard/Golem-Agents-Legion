> 🌐 **English** · [繁體中文](docs/i18n/zh-Hant/README.zh-Hant.md)

# Golem-Agents-Legion

A document-driven AI working system for solo developers who want one stable workflow across GitHub Copilot, Antigravity CLI, Codex CLI, and Claude Code. GAL keeps planning, implementation, testing, review, and research in repo-owned Markdown so work moves between AI tools without losing context.

This README is the map. It links out to the docs that own each topic in depth — it does not repeat them.

## What is GAL

- A small **control-plane** command surface (`/gal …`) plus a set of specialized **golem agents**.
- **Repo-owned state** (`.dev/`, `docs/plans/`, `docs/research/`) instead of provider-local chat memory, so any tool can resume where another stopped.
- **Cross-runtime parity**: the same contracts drive every supported AI CLI.

## Quick Start

GAL's primary path is **install mode** — you do not need to clone this repo.

1. **Install the `gal` CLI** (install mode; see [Install Status](#install-status) for what is shipped today).
2. **In your project**, open a supported runtime and run `/gal init` (Codex uses `$gal init`).
3. **Work the flow**: `/planning` → `/gal pipeline` → `/gal wrap-up`.

Detailed first-time setup, machine config, and daily operations live in the **[user manual](docs/manual.md)**.

## Dev Mode

To work on GAL itself (contributor / source mode), use a local clone:

```bash
git clone https://github.com/monkey1wizard/golem-agents-legion.git
# Windows:           ./scripts/Setup-Machine.ps1
# macOS / Linux:     ./scripts/setup-machine.sh
```

This is the source-mode contributor path. Full dev setup, runtime topology, and the `~/.gal/` layout are in the **[developer guide](docs/devguide.md)**.

## Install Status

Plugin/install mode is rolling out per runtime — stated honestly:

| Runtime | Install status |
| --- | --- |
| Antigravity CLI (AGY) | ✓ install lifecycle verified |
| Claude Code | plugin artifact + local marketplace verified; public direct-install still gated |
| Codex CLI · Copilot CLI | discoverability-first; native direct-install **deferred** |
| `winget` · `homebrew` · GitHub Releases | release contract defined; package-manager lanes **not yet shipped end to end** |

GitHub Releases is always the canonical version source and fallback. The full release lineage, marketplace matrix, and lag policy live in [developer guide → Release Artifact Matrix](docs/devguide.md#release-artifact-matrix).

## Distribution & Migration

When moving machines or rebuilding GAL, back up only the machine intent you cannot regenerate (`~/.gal/config/config.json`, `~/.gal/config/xmachine.json`, `~/.gal/state/plugins.lock.json`, local overrides, secrets). The package payload and `~/.gal/generated/` projections rebuild on reinstall.

Step-by-step backup, migration, and uninstall boundaries are in [user manual → Machine Operations](docs/manual.md#machine-operations). Package-manager uninstall removes only the `gal` binary and never deletes user-owned config; a full reset is a separate explicit purge flow.

## How GAL Works

A one-glance mental model. Each row links to the doc that owns the full contract.

### Feature lifecycle

```text
/gal init → /planning → /deep-planning → /refining-plan → /plan-to-prompt → /gal pipeline → /gal wrap-up
```

`/planning` writes a human-readable plan to `docs/plans/`. `/deep-planning` runs architect review. `/refining-plan` locks `## Tasks` + `## Test Plan`. `/plan-to-prompt` produces the execution work file. `/gal pipeline` chains implement → test → review per task. `/gal wrap-up` records continuity so the next session (or tool) resumes cleanly.

### Commands

`/gal init · status · whats-next · wrap-up · research · deep-research · pipeline` plus `/planning · deep-planning · refining-plan · plan-to-prompt`. Full contract: [commands/commands.md](commands/commands.md).

### Golem agents

12 specialized agents in three categories — **Utility** (debugger, notewriter), **Domain** (architect, analyst, designer, researcher, security, releaser), **Pipeline** (implementer, tester, reviewer, verifier). Independent tester/reviewer/verifier (different models) keep verification credible. Full roster and rules: [agent/agents.md](agent/agents.md) and [workflows/coding.md](workflows/coding.md).

### Pipeline

```text
T-NNN → implementer → tester → reviewer → [conditional security] → git commit → next task
                                  └── REJECT → auto-fix ──┘
all tasks done → verifier → confirm goal
```

`golem-security` is inserted only for auth / data / input / public-API / trust-boundary changes. The full workflow semantics live in [workflows/coding.md](workflows/coding.md).

### Research

A workflow parallel to development. `/gal research` (RESEARCH → VERIFY → DOCUMENT) and `/gal deep-research` (adds SYNTHESIZE → CROSS-REVIEW, ≥5 sources). VERIFY must use a different model from the author. Default output is `docs/research/`; can route to private notes, knowledge capture, or none.

## Files & Storage

GAL's durable memory is repo-owned and reviewable:

```text
.dev/
├── project.md                 compressed project summary (cold-start first read; the only .dev file committed in this repo)
├── state.md                   active plans index + session continuity
└── plans/<slug>.prompt.md     AI execution work file (mutable task memory)
docs/plans/<slug>.md           human-readable source plan (transient)
docs/research/                 durable research output
~/.gal/                        machine-local runtime (config, plugins, generated) — see developer guide
```

Private notes (Obsidian Vault) are a separate, user-owned boundary configured in `~/.gal/config/config.local.env`; the `notewriter` agent follows your Guide when set, generic mode otherwise.

## Collaborative Tools

Optional external tools that enhance specific lanes. **GAL never auto-installs them and works fully without any of them**; each goes through a shared preflight (`applicable → available → ready → route / degrade`).

| Tool | What it does | Doc |
| --- | --- | --- |
| graphify | Converts repo files into a knowledge graph (`graphify-out/`); GAL consumes it during planning and review to surface cross-module coupling | [→](docs/collaborative-tools/graphify.md) |
| Playwright MCP | Managed browser capability for test runs, design audits, dynamic-page research, and browser-visible MCP evaluation; isolated and headless by default | [→](docs/collaborative-tools/playwright-mcp.md) |
| OpenCLI | Turns websites, browser sessions, Electron apps, and local tools into reusable CLI commands; reuses logged-in sessions | [→](docs/collaborative-tools/opencli.md) |
| gstack | Garry Tan's (YC) startup-experience AI agents; adds optional planning-review and engineering-review lanes to the GAL planning flow | [→](docs/collaborative-tools/gstack.md) |
| xmachine | Routes bounded work over SSH to readied work nodes; covers Windows dispatch and POSIX detached execution | [→](docs/collaborative-tools/xmachine.md) |
| Godot C# | Commands, conventions, skills, and MCP tools for Godot 4 C# repos *(WIP)* | [→](docs/collaborative-tools/godot.md) |
| Blender MCP | Setup checks and integration notes for the `blender-mcp` server | [→](docs/collaborative-tools/blender-mcp.md) |
| AI-first game assets | ComfyUI-first asset generation pipeline with downstream organization and export tools *(WIP)* | [→](docs/collaborative-tools/graphics-workflow.md) |

Preflight contract: [checking-contract](docs/collaborative-tools/checking-contract.md).

## Learn More

| Doc | What it owns |
| --- | --- |
| [user manual](docs/manual.md) | machine setup, runtime selection, config, model routing, MCP, Obsidian, working hours, backup/uninstall — everything for running GAL |
| [developer guide](docs/devguide.md) | maintainer: codebase + `~/.gal/` structure, distribution & release, making changes |
| [commands/commands.md](commands/commands.md) · [agent/agents.md](agent/agents.md) · [workflows/coding.md](workflows/coding.md) | canonical control-plane, agent, and workflow contracts |
| [docs/collaborative-tools/](docs/collaborative-tools/) | one contract per optional tool |

## References

- [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done)
- [GitHub Spec Kit](https://github.com/github/spec-kit)
- [gstack](https://github.com/garrytan/gstack)
- [rtk](https://github.com/rtk-ai/rtk): filters and compresses command outputs before they reach LLM context. Strongly recommended.

## License

MIT — See [LICENSE](LICENSE).
