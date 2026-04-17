# Golem Agents Legion (GAL)

English | [繁體中文](README.zh-Hant.md)

GAL is a Markdown-native AI working system with two layers:

- A `/gal` control plane for repo bootstrap, state inspection, next-action routing, wrap-up, and research
- A specialist execution layer with a GAL-native planning family, provider-routed planning review lanes, QA, release, memory, and guardrails

The point is not to preserve one tool's UX. The point is to keep the methodology, state model, and command contracts under your control while letting Copilot and Gemini execute the same workflow.

## What GAL Is

GAL separates durable workflow knowledge from tool-specific adapters.

- Knowledge lives in this repo as Markdown: workflows, agents, conventions, templates, and skills
- Repo-local execution state lives in `.dev/`, with source plans under `docs/plans/`
- Tool adapters are generated outputs, not the source of truth

This repo is not an application service. It is the canonical methodology and command surface.

## Lineage

GAL draws from several adjacent systems rather than a single upstream.

- [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done) contributes the phase-based workflow discipline: explicit state, verification gates, and a more rigorous execution lifecycle.
- [gstack](https://github.com/garrytan/gstack) contributes much of the specialist workflow semantics and command vocabulary, GAL reimplements those semantics natively instead of depending on upstream gstack runtime or storage.
- [GitHub Spec Kit](https://github.com/github/spec-kit) contributes the portable command-kit and artifact-driven direction: repo-carried workflow artifacts, Markdown-native docs, and installable command surfaces across different runtimes.

GAL's own addition is the split between a dedicated `/gal` control plane and a specialist execution layer, backed by a repo-local canonical state model under `.dev/`, `docs/plans/`, and related artifact folders.

In short: GAL is not a fork of any one source. It recombines ideas from GSD, gstack, and Spec Kit into a Copilot/Gemini/Codex-friendly operating model.

## Final Operating Model

GAL now uses a strict split between control plane and execution layer.

| Layer | Responsibility | Commands |
| --- | --- | --- |
| Control plane | Bootstrap repo, read state, recommend next action, converge continuity, route research | `/gal init`, `/gal status`, `/gal whats-next`, `/gal wrap-up`, `/gal research` |
| Specialist execution | Plan, provider-routed planning reviews, design, debug, review, QA, release, memory, guardrails | `/planning`, `/deep-planning`, `/plan-to-prompt`, `/review`, `/qa`, `/ship`, and the rest of the specialist catalog below |

`/gal` does not duplicate specialist behavior. Specialist commands write back to canonical artifacts that `/gal` reads.

## Canonical Artifacts

These files are the durable state model.

| Path | Purpose |
| --- | --- |
| `.dev/project.md` | Repo summary, stack, goals, constraints |
| `.dev/state.md` | Active plan index, blockers, session continuity |
| `docs/plans/<plan-slug>.md` | Human-readable source plan doc (scope, rationale, requirements) |
| `.dev/plans/<plan-slug>.prompt.md` | AI execution work file — mutable checklist, phase markers, execution status, write-back target |
| `DESIGN.md` | Repo-level design governance (design system, not plan-specific) |
| `CLAUDE.md` | Repo-local operational notes such as deploy config and design references |
| `docs/designs/<plan-slug>/` | Plan-bound design assets: `variant-approved.json`, `variant-approved.png`, `handoff-final.html` |
| `docs/qa-reports/` | QA reports: `YYYYMMDD-<plan-slug>.md` / `YYYYMMDD-<plan-slug>-report-only.md` |
| `docs/design-reports/` | Design audit reports: `YYYYMMDD-<plan-slug>-rNN.md` |
| `docs/research/` | Research notes: `YYYYMMDD-<plan-slug>-<topic>.md` |
| `.dev/learnings.jsonl` | Repo-local institutional memory |

### Plan Execution Sections

The execution work file (`.prompt.md`) contains three specialist-written sections with explicit ownership rules:

| Section | Written by | Consumed by | Purpose |
| --- | --- | --- | --- |
| `## Open Questions` | `/planning` initializes them in the source plan, planning-stage review lanes append, `/plan-to-prompt` carries them into execution | `/ship`, `/gal status`, `/gal whats-next` | Stable list of unresolved assumptions and decisions, IDs `OQ-NNN` |
| `## Tasks` | Engineering review lane initializes them in the source plan after Eng Review CLEAR; `/plan-to-prompt` carries them into execution; implementation marks completion only | `/review`, `/qa`, `/ship`, `/gal status`, `/gal whats-next` | Verifiable task checklist, IDs `T-NNN` |
| `## Analyze` | `/review` (sole writer, verdict: `CLEAR` / `DRIFT-OPEN` / `NOT-RUN`) | `/ship`, `/gal status`, `/gal whats-next` | Drift check: did the diff stay within plan scope? |

Consumers read these sections for display and routing — they do not recalculate or overwrite them.

## Quick Start

### Windows

```powershell
git clone https://github.com/monkey1wizard/golem-agents-legion.git ~/golem-agents-legion
~/golem-agents-legion/scripts/Setup-Machine.ps1
```

### macOS

```bash
git clone https://github.com/monkey1wizard/golem-agents-legion.git ~/golem-agents-legion
~/golem-agents-legion/scripts/setup-machine.sh
```

Then, inside a target repo:

```text
# Copilot / Gemini CLI (slash-command surface)
/gal init
/planning
/deep-planning
/plan-to-prompt
/gal whats-next

# Codex CLI (skill mention surface — $ prefix, not /)
$gal init
$planning
$deep-planning
$plan-to-prompt
$gal whats-next
```

There is no public `gal sync` step in the final model. Adapter generation is installation plumbing, not a user workflow.

`Setup-Machine.ps1` and `setup-machine.sh` also merge a VS Code user setting so Copilot Chat ignores `~/.agents/skills`. This keeps the shared reusable-skill install from showing up twice in VS Code while Gemini uses native commands from `~/.gemini/commands` and Codex uses command skills from `~/.codex/skills`.

`Setup-Machine.ps1` now checks whether `rg` (ripgrep) is available. On Windows it refreshes `PATH`, warns if ripgrep is already installed but the current shell cannot see it yet, and otherwise offers to install ripgrep via `winget`. The Unix setup script follows the same pattern with the first supported package manager it finds.

The setup scripts also merge a canonical MCP catalog from `mcp-servers.example.json` plus optional local overrides from `mcp-servers.local.json` into provider-owned config files for VS Code, Gemini CLI, and Codex CLI.

## Using GAL For AI-First Game Asset Production

GAL can also drive an AI-first workflow for producing game-ready 2D and 3D assets.

The intent is not to integrate every graphics app equally. The workflow starts from ComfyUI for generation and variation, then routes assets through the smallest useful finishing tool for the asset lane.

### Supported Asset Lanes

| Lane | Default Flow |
| --- | --- |
| 2D concept and illustration | ComfyUI -> GIMP |
| Sprite and pixel assets | ComfyUI -> Aseprite |
| UI, icon, and HUD assets | ComfyUI -> Figma -> Inkscape |
| 3D game assets | ComfyUI -> Blender |

### Recommended Tool Stack

- `joenorton/comfyui-mcp-server` as the generation layer
- `maorcc/gimp-mcp` for raster cleanup and export
- `willibrandon/pixel-mcp` for sprite, animation, and spritesheet workflows
- `grab/cursor-talk-to-figma-mcp` for UI and HUD layout work
- `grumpydevorg/inkscape-mcps` for SVG cleanup and deterministic export
- `ahujasid/blender-mcp` for general 3D asset work

### What GAL Now Knows For Game Assets

The repo now includes game-asset workflow guidance in:

- [docs/graphics-workflow.md](docs/graphics-workflow.md) for lane routing, handoff rules, and output contracts
- [docs/graphics-mcp-setup.md](docs/graphics-mcp-setup.md) for MCP stack setup
- [docs/graphics-external-knowledge.md](docs/graphics-external-knowledge.md) for official documentation references
- skills under `skills/graphics-workflow` and `skills/game-*` for lane-specific execution

### Game Asset Session Rhythm

```text
/gal init
/planning
/deep-planning
/plan-to-prompt

# Then implement with the game asset skills:
# - graphics-workflow
# - game-2d-assets
# - game-pixel-assets
# - game-ui-assets
# - game-3d-assets
# - game-asset-export
```

Keep ComfyUI as the default entry point. Treat Blender, GIMP, Aseprite, Figma, and Inkscape as finishing and export tools, not as alternate generation centers.

## Using GAL With Godot C Sharp

GAL does not add a new Godot-specific agent. It teaches the existing planning, implementation, review, and QA commands how to work against a Godot 4 C# repo through conventions, skills, and MCP tools.

### What To Install

For a Godot C# project, the default stack is:

- Godot 4.x with C# support
- VS Code C# tooling for IntelliSense and diagnostics
- `Coding-Solo/godot-mcp` for editor launch, run control, and debug output
- `n24q02m/better-godot-mcp` for offline `.tscn` and resource editing
- `MingHuiLiu/godot4-runtime-mcp` for runtime scene-tree, signal, log, and screenshot inspection

See [docs/godot-mcp-setup.md](docs/godot-mcp-setup.md) for the tool split and installation notes.

### What GAL Now Knows

The repo now includes Godot-specific guidance in:

- [conventions/csharp.md](conventions/csharp.md) for Godot runtime rules, `partial class`, signals, exports, and lifecycle methods
- [docs/godot-external-knowledge.md](docs/godot-external-knowledge.md) for official Godot C# documentation references
- Godot skills under `skills/godot-*` for project ops, scene authoring, scripting, runtime debugging, and asset pipeline work

### How To Start In A Godot Repo

Inside the target Godot C# repo:

```text
/gal init
/planning
/deep-planning
/plan-to-prompt
```

If you are using Codex CLI instead of slash commands:

```text
$gal init
$planning
$deep-planning
$plan-to-prompt
```

`/gal init` should detect a Godot C# repo from `project.godot` plus `*.csproj` and write that into `.dev/project.md`.

### Recommended Godot Workflow

Use GAL like this for normal feature work:

```text
/gal init
/planning
/deep-planning
/plan-to-prompt

# Then implement with the Godot skills and tools:
# - godot-project-ops
# - godot-scene-authoring
# - godot-scripting
# - godot-runtime-debug
# - godot-asset-pipeline

/review
/qa
/ship
```

### Tool Selection Rules

Use the tools by responsibility:

- Build, import, export, and CI automation: Godot CLI
- Launch the editor, run the project, capture debug output: `godot-mcp`
- Edit scenes and resources without a running editor: `better-godot-mcp`
- Inspect live nodes, signals, logs, and runtime state: `godot4-runtime-mcp`

### Godot CLI Examples

```bash
godot --headless --path <project> --build-solutions
godot --headless --path <project> --import
godot --headless --path <project> --export-release <preset> <output>
godot --path <project> -e
godot --path <project>
```

### Important Constraint

Godot gameplay code does not follow the same compatibility target as GAL's external tooling.

- Godot runtime code should stay on the version the checked-in Godot project supports, typically `.NET 8 / C# 12`
- External tools and MCP servers may use newer runtimes because Godot does not load them

When writing gameplay code, follow the Godot runtime section in [conventions/csharp.md](conventions/csharp.md).

### Recommended Session Rhythm

Use the control plane like this:

```text
/gal init
/planning
/deep-planning
/plan-to-prompt
/gal whats-next
/gal pipeline        # iterates T-NNN tasks: implement → commit → test → review, final verifier
/ship
```

Or if you prefer manual control:

```text
/gal init
/planning
/deep-planning
/plan-to-prompt
/gal whats-next
<implement>
/review
/qa
/ship
```

When you resume later, start with:

```text
/gal status
# or
/gal whats-next
```

`/gal wrap-up` is required before ending a work session, and recommended after any meaningful checkpoint you may need to resume cleanly later — for example after finishing `T-001`, before a context switch, or before handing work to another model or session.

## Control-Plane Commands

These are the stable user-facing `/gal` commands.

> **CLI invocation note** — Copilot CLI and Gemini CLI expose these as `/gal <subcommand>`. Codex CLI uses `$gal <subcommand>` instead. The `/` prefix in Codex is reserved for built-in Codex commands only.

| Command | When To Use | Reads | Writes | Outcome |
| --- | --- | --- | --- | --- |
| `/gal init` | Bootstrap a repo for GAL | Existing repo docs and structure | `.dev/project.md`, `.dev/state.md` | Repo is ready for GAL-managed work |
| `/gal status` | You need full state projection | `.dev/state.md`, active execution plan files (`.prompt.md`) | Nothing | Reports active plans, review/test status, blockers, continuity, readiness |
| `/gal whats-next` | You want a single next action | `.dev/state.md`, active execution plan status and results | Nothing | Returns one recommended next command or task |
| `/gal wrap-up` | You are ending a session, or pausing at a meaningful checkpoint | `.dev/state.md`, active plan | `### Handoff Notes`, `## Session Continuity` | Converges resumable context |
| `/gal research` | You need structured investigation | Current repo context | Research artifacts as directed | Enters research workflow |
| `/gal pipeline` | Task-driven autopilot: iterate every T-NNN task (implement → commit → test → review), with a final verifier pass. Optional `from T-NNN` / `stop-at T-NNN` boundaries. | Active plan `## Tasks`, `## Test Plan`, `model-roles.local.md` | Plan `## Status` (Current Task, Task Base/Final Commit, retry counts), `## Test Results`, `## Review Results` | Iterates all tasks automatically; stops only on security blocker, retry ceiling (3), curfew, stop-at boundary, or verifier gap |

### Discoverability Aliases

After `Setup-Machine`, these exist as slash commands in Copilot and Gemini, and as named skills in Codex. Gemini reads generated native commands from `~/.gemini/commands`, while Codex reads the same command directories from `~/.codex/skills`.

| Alias | Copilot / Gemini | Codex CLI |
| --- | --- | --- |
| gal-init | `/gal-init` | `$gal-init` |
| gal-status | `/gal-status` | `$gal-status` |
| gal-whats-next | `/gal-whats-next` | `$gal-whats-next` |
| gal-wrap-up | `/gal-wrap-up` | `$gal-wrap-up` |
| gal-pipeline | `/gal-pipeline` | `$gal-pipeline` |

## Specialist Command Catalog

These commands implement the work layer directly. They do not route through `/gal`.

### Planning

| Command or Lane | Purpose | Primary Writes |
| --- | --- | --- |
| `/planning` | Create or replace a human-readable source plan | `docs/plans/<plan-slug>.md`, `.dev/state.md` |
| `/deep-planning` | Refine planning-stage material into an implementation-ready source plan | `docs/plans/<plan-slug>.md`, `.dev/state.md` |
| Business, design, and engineering review lanes | Provider-routed planning-stage deep-planning passes; use upstream gstack if installed, otherwise fallback golems via `/gal golem-analyst`, `/gal golem-designer`, `/gal golem-architect` | Source plan `## Review Results`, `## Open Questions`, and for engineering also `## Test Plan`, `## Tasks`, `<!-- ENG_REVIEW: CLEAR -->` |
| `/plan-to-prompt` | Materialize the mutable execution prompt from the reviewed source plan right before implementation | `.dev/plans/<plan-slug>.prompt.md`, `.dev/state.md` |
| `/cso` | OWASP plus STRIDE security review | Plan `## Review Results` |

### Design

| Command | Purpose | Primary Writes |
| --- | --- | --- |
| `/design-consultation` | Creates the product design system | `DESIGN.md`, `CLAUDE.md` |
| `/design-shotgun` | Generates multiple visual variants and records approval | `docs/designs/<plan-slug>/variant-approved.json` |
| `/design-html` | Converts an approved design into runnable HTML or component code | `docs/designs/<plan-slug>/handoff-final.html` |
| `/design-review` | Live-site audit against `DESIGN.md` with surgical fixes | Plan `## Review Results`, `docs/design-reports/` |

### Debug And Review

| Command | Purpose | Primary Writes |
| --- | --- | --- |
| `/investigate` | Root-cause-first debugging workflow | Plan `## Debug Session` |
| `/review` | Staff-level diff review for bugs CI misses | Plan `## Review Results`, `## Analyze` |

### Browser And QA

| Command | Purpose | Primary Writes |
| --- | --- | --- |
| `/browse` | Playwright browser capability primitive used by other commands | Session only |
| `/connect-chrome` | Switches browser work to headed Chrome | Session only |
| `/setup-browser-cookies` | Imports real browser auth into Playwright | Session only |
| `/qa` | Full QA pass with fix loop and regression tests | Plan `## Test Results`, `docs/qa-reports/` |
| `/qa-only` | QA bug report without code changes | Plan `## Test Results (Report Only)`, `docs/qa-reports/` |

### Ship And Release

| Command | Purpose | Primary Writes |
| --- | --- | --- |
| `/ship` | Final pre-merge gate: tests, coverage, PR, docs | Plan `## Ship` |
| `/land-and-deploy` | Merge and verify production deployment | Plan `## Deploy` |
| `/setup-deploy` | One-time deploy configuration | `CLAUDE.md` |
| `/document-release` | Updates docs to match shipped code | Repo docs, PR body |

### Memory And Guardrails

| Command | Purpose | Primary Writes |
| --- | --- | --- |
| `/learn` | Repo-local institutional memory manager | `.dev/learnings.jsonl` |
| `/careful` | Warns before destructive commands | Session only |
| `/freeze` | Restricts edits to a directory boundary | Session only |
| `/guard` | Combines `/careful` and `/freeze` | Session only |
| `/unfreeze` | Removes the active freeze boundary | Session only |
| `/gstack-upgrade` | Pulls latest GAL and re-runs setup | Machine maintenance only |

## Typical Flow

### New feature

```text
/gal init
/planning
/deep-planning
/plan-to-prompt
/gal whats-next
/gal pipeline        # task-by-task: implement → commit → test → review (multi-vendor AI)
/ship
```

Or step by step:

```text
/gal init
/planning
/deep-planning
/plan-to-prompt
/gal whats-next
<implement>
/gal wrap-up
/review
/qa
/ship
```

### Bug investigation

```text
/gal status
/investigate
/review
/qa
/gal wrap-up
```

### Production release

```text
/ship
/land-and-deploy
```

## State Logic

The control plane works because specialist commands write predictable sections back to the active plan.

| Section | Written By | Read By |
| --- | --- | --- |
| `## Review Results` | Review specialists | `/gal status`, `/gal whats-next` |
| `## Test Plan` | Engineering review lane in the source plan, then `/plan-to-prompt` carries it into execution | `/qa`, `/qa-only` |
| `## Test Results` | `/qa`, `/qa-only` | `/gal status`, `/gal whats-next` |
| `## Ship` | `/ship` | `/gal status`, `/gal whats-next`, `/land-and-deploy` |
| `## Deploy` | `/land-and-deploy` | `/gal status`, `/gal whats-next` |
| `### Handoff Notes` | `/gal wrap-up` | `/gal status`, `/gal whats-next` |

### Troubleshooting State Detection

- If `.dev/state.md` is missing, the repo is not initialized yet.
- If `.dev/state.md` exists, GAL should read the active workflow from `.dev/plans/<plan-slug>.prompt.md` `## Status`.
- If GAL cannot project state even though `.dev/state.md` exists, treat that as a malformed state issue, not a signal to re-run `/gal init`.

### Troubleshooting Duplicate Skills In VS Code

- VS Code currently scans both `~/.copilot/skills` and `~/.agents/skills`.
- GAL intentionally installs shared reusable skills under `~/.agents/skills`, so an unconfigured VS Code instance can show duplicate skill entries.
- Re-run `scripts/Setup-Machine.ps1` or `scripts/setup-machine.sh` to let the installer merge the recommended VS Code setting automatically.
- If you need to repair an existing install by hand, add this to your VS Code user `settings.json`:

```json
"chat.agentSkillsLocations": {
  "~/.agents/skills": false
}
```

- This only tells VS Code to ignore the duplicate path. It does not remove `~/.agents/skills`, so shared reusable skills keep working for Gemini CLI and Codex CLI.

## Architecture Summary

```text
~/golem-agents-legion/     canonical methodology and command source
<repo>/.dev/              repo-local state and continuity
docs/plans/<slug>.md      human-readable source plan doc
.dev/plans/<slug>.prompt.md  AI execution work file (mutable state)
mcp-servers.example.json  tracked MCP source of truth
mcp-servers.local.json    local MCP enable/override layer (gitignored)
~/.copilot/skills/        installed Copilot skills
~/.gemini/skills/         legacy Gemini runtime dir (cleaned up by setup)
~/.gemini/commands/       generated Gemini native slash commands
~/.agents/skills/         shared reusable skills (VS Code should ignore this path)
~/.codex/skills/          installed Codex command skills
%APPDATA%/Code/User/mcp.json   VS Code MCP config merged by setup
~/.gemini/settings.json   Gemini settings + mcpServers merged by setup
~/.codex/config.toml      Codex config + [mcp_servers.*] merged by setup
```

The methodology is portable. The adapters are disposable.

## Personalization

Several files still contain environment-specific placeholders you must set after cloning.

| Placeholder | Meaning | Files |
| --- | --- | --- |
| `<OBSIDIAN_VAULT>` | Absolute path to your Obsidian vault | Obsidian agents and skills |
| `<OBSIDIAN_VAULT_NAME>` | Vault name shown in Obsidian | Obsidian skills |
| `<LOCAL_SEARCH_PROJECT>` | Path to your local search project clone | Local-first and knowledge-management skills |
| `<GAL_SKILLS>` | Path where skills are installed | Some helper skills |
| `<TEMP_DIR>` | Temp output directory | PDF skill |
| `<MCP_FILESYSTEM_PATHS>` | Optional comma-separated filesystem roots for the filesystem MCP | MCP manifest merge |
| `<MCP_MEMORY_FILE_PATH>` | Path to the persistent MCP memory JSON file | MCP manifest merge |
| `<CONTEXT7_API_KEY>` | Optional Context7 API key for runtimes that need it | MCP manifest merge |
| `<OBSIDIAN_API_KEY>` | Obsidian Local REST API key | Obsidian MCP |
| `<OBSIDIAN_BASE_URL>` | Obsidian Local REST API base URL | Obsidian MCP |

For model routing, copy [model-roles.example.md](model-roles.example.md) to `model-roles.local.md` and customize it.

If you need provider-specific MCP differences, edit `mcp-servers.local.json` and rerun Setup-Machine. Use `config.local.env` for local secrets and path values referenced by the manifest.

Optional external CLIs such as OpenCLI or Defuddle remain skill-layer dependencies. GAL does not impose a universal CLI-first rule; each skill defines its preferred tool order, fallback path, and no-tool behavior.

## Important Docs

| Path | Purpose |
| --- | --- |
| [docs/ai-agent-onboarding.md](docs/ai-agent-onboarding.md) | Reading order for AI agents and maintainers |
| [docs/gal-control-plane-contracts.md](docs/gal-control-plane-contracts.md) | Canonical `/gal` read/write contracts |
| [docs/gstack-integration.md](docs/gstack-integration.md) | How GAL routes optional gstack providers without exposing duplicate public commands |
| [docs/gstack-command-contracts.md](docs/gstack-command-contracts.md) | Provider and specialist implementation blueprint |
| [docs/godot-mcp-setup.md](docs/godot-mcp-setup.md) | Recommended Godot C# MCP stack and tool-selection guide |
| [docs/godot-external-knowledge.md](docs/godot-external-knowledge.md) | Official Godot C# references and source-of-truth links |
| [docs/runtime-verification.md](docs/runtime-verification.md) | Live/manual verification status for commands and execution-plane behavior |
| [docs/command-dispatch-architecture.md](docs/command-dispatch-architecture.md) | Dispatch model and alias policy |
| [commands/commands.md](commands/commands.md) | Installed command surface and alias architecture |
| [workflows/coding.md](workflows/coding.md) | Command-driven coding flow, lifecycle rules, and review expectations |

## What GAL No Longer Treats As Public Workflow

- Legacy tiered risk labels are no longer part of the command surface or planning model
- Upstream gstack installation is not required to use GAL's specialist commands

## License

MIT — see [LICENSE](LICENSE).
