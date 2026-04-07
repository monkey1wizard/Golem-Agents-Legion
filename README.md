# Golem Agents Legion (GAL)

English | [繁體中文](README.zh-Hant.md)

GAL is a Markdown-native AI working system with two layers:

- A `/gal` control plane for repo bootstrap, state inspection, next-action routing, wrap-up, and research
- A gstack-style specialist command surface for planning, review, QA, release, memory, and guardrails

The point is not to preserve one tool's UX. The point is to keep the methodology, state model, and command contracts under your control while letting Copilot and Gemini execute the same workflow.

## What GAL Is

GAL separates durable workflow knowledge from tool-specific adapters.

- Knowledge lives in this repo as Markdown: workflows, agents, conventions, templates, and skills
- Repo-local execution state lives in `.dev/` and `docs/plans/`
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
| Specialist execution | Plan, design, debug, review, QA, release, memory, guardrails | `/office-hours`, `/plan-eng-review`, `/review`, `/qa`, `/ship`, and the rest of the specialist catalog below |

`/gal` does not duplicate specialist behavior. Specialist commands write back to canonical artifacts that `/gal` reads.

## Canonical Artifacts

These files are the durable state model.

| Path | Purpose |
| --- | --- |
| `.dev/project.md` | Repo summary, stack, goals, constraints |
| `.dev/state.md` | Active plan index, blockers, session continuity |
| `docs/plans/<plan-slug>.md` | Human-readable source plan doc (scope, rationale, requirements) |
| `docs/plans/<plan-slug>.prompt.md` | AI execution work file — mutable checklist, workflow state, execution status, write-back target |
| `DESIGN.md` | Repo-level design governance (design system, not plan-specific) |
| `CLAUDE.md` | Repo-local operational notes such as deploy config and design references |
| `docs/designs/<plan-slug>/` | Plan-bound design assets: `variant-approved.json`, `variant-approved.png`, `handoff-final.html` |
| `docs/qa-reports/` | QA reports: `YYYYMMDD-<plan-slug>.md` / `YYYYMMDD-<plan-slug>-report-only.md` |
| `docs/design-reports/` | Design audit reports: `YYYYMMDD-<plan-slug>-rNN.md` |
| `docs/benchmarks/` | Performance baselines: `YYYYMMDD-HHmmss-<url-slug>.json`, canary baselines: `canary-YYYYMMDD-HHmmss-<url-slug>.json` |
| `docs/retros/` | Retro snapshots: `YYYYMMDD.json` |
| `docs/research/` | Research notes: `YYYYMMDD-<plan-slug>-<topic>.md` |
| `.dev/learnings.jsonl` | Repo-local institutional memory |

### Plan Execution Sections

The execution work file (`.prompt.md`) contains three specialist-written sections with explicit ownership rules:

| Section | Written by | Consumed by | Purpose |
| --- | --- | --- | --- |
| `## Open Questions` | `/office-hours` (initial), `/plan-ceo-review`, `/plan-design-review` (append), `/plan-eng-review` closes resolved items | `/ship`, `/gal status`, `/gal whats-next` | Stable list of unresolved assumptions and decisions, IDs `OQ-NNN` |
| `## Tasks` | `/plan-eng-review` (sole initializer after Eng Review CLEAR), implementation marks completion only | `/review`, `/qa`, `/ship`, `/gal status`, `/gal whats-next` | Verifiable task checklist, IDs `T-NNN` |
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
/gal status
/office-hours
/autoplan

# Codex CLI (skill mention surface — $ prefix, not /)
$gal init
$gal status
$office-hours
$autoplan
```

There is no public `gal sync` step in the final model. Adapter generation is installation plumbing, not a user workflow.

`Setup-Machine.ps1` and `setup-machine.sh` also merge a VS Code user setting so Copilot Chat ignores `~/.agents/skills`. This keeps Gemini/Codex using the shared `.agents` install while preventing duplicate skill entries in VS Code.

### Recommended Session Rhythm

Use the control plane like this:

```text
/gal init
/office-hours
/autoplan
<implement>
/gal wrap-up
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

### Discoverability Aliases

These exist for slash-command autocomplete in Copilot/Gemini, and as named skills in Codex.

| Alias | Copilot / Gemini | Codex CLI |
| --- | --- | --- |
| gal-init | `/gal-init` | `$gal-init` |
| gal-status | `/gal-status` | `$gal-status` |
| gal-whats-next | `/gal-whats-next` | `$gal-whats-next` |
| gal-wrap-up | `/gal-wrap-up` | `$gal-wrap-up` |

## Specialist Command Catalog

These commands implement the work layer directly. They do not route through `/gal`.

### Planning

| Command | Purpose | Primary Writes |
| --- | --- | --- |
| `/office-hours` | YC-style sprint or feature kickoff that creates a new plan | New `docs/plans/<plan-slug>.md` + `.prompt.md`, `.dev/state.md`, initial `## Open Questions` |
| `/plan-ceo-review` | Scope and ambition review from a founder perspective | Plan `## Review Results`, `## Open Questions` (scope OQs) |
| `/plan-eng-review` | Architecture and test-plan gate, required before `/ship` | Plan `## Review Results`, `## Test Plan`, `## Tasks`, closes resolved `## Open Questions` |
| `/plan-design-review` | Pre-implementation UX and design audit | Plan `## Review Results`, `## Open Questions` (design OQs) |
| `/autoplan` | Chains CEO, design, and eng reviews with auto-decisions | Plan review sections and test plan |
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
| `/canary` | Post-deploy monitoring against production | Baselines, optional plan note |
| `/benchmark` | Real-browser performance measurement and comparison | `docs/benchmarks/`, optional plan `## Performance` |
| `/setup-deploy` | One-time deploy configuration | `CLAUDE.md` |
| `/document-release` | Updates docs to match shipped code | Repo docs, PR body |
| `/retro` | Retrospective with repo metrics and snapshots | `docs/retros/` |

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
/office-hours
/autoplan
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
/canary
/retro
```

## State Logic

The control plane works because specialist commands write predictable sections back to the active plan.

| Section | Written By | Read By |
| --- | --- | --- |
| `## Review Results` | Review specialists | `/gal status`, `/gal whats-next` |
| `## Test Plan` | `/plan-eng-review` | `/qa`, `/qa-only` |
| `## Test Results` | `/qa`, `/qa-only` | `/gal status`, `/gal whats-next` |
| `## Ship` | `/ship` | `/gal status`, `/gal whats-next`, `/land-and-deploy` |
| `## Deploy` | `/land-and-deploy` | `/gal status`, `/canary` |
| `### Handoff Notes` | `/gal wrap-up` | `/gal status`, `/gal whats-next` |

### Troubleshooting State Detection

- If `.dev/state.md` is missing, the repo is not initialized yet.
- If `.dev/state.md` exists, GAL should read the active workflow from `docs/plans/<plan-slug>.prompt.md` `## Status`.
- If GAL cannot project state even though `.dev/state.md` exists, treat that as a malformed state issue, not a signal to re-run `/gal init`.

### Troubleshooting Duplicate Skills In VS Code

- VS Code currently scans both `~/.copilot/skills` and `~/.agents/skills`.
- GAL intentionally installs shared Gemini/Codex skills under `~/.agents/skills`, so an unconfigured VS Code instance can show duplicate entries such as `/gal-status`.
- Re-run `scripts/Setup-Machine.ps1` or `scripts/setup-machine.sh` to let the installer merge the recommended VS Code setting automatically.
- If you need to repair an existing install by hand, add this to your VS Code user `settings.json`:

```json
"chat.agentSkillsLocations": {
  "~/.agents/skills": false
}
```

- This only tells VS Code to ignore the duplicate path. It does not remove `~/.agents/skills`, so Gemini CLI and Codex CLI keep working.

## Architecture Summary

```text
~/golem-agents-legion/     canonical methodology and command source
<repo>/.dev/              repo-local state and continuity
docs/plans/<slug>.md      human-readable source plan doc
docs/plans/<slug>.prompt.md  AI execution work file (mutable state)
~/.copilot/skills/        installed Copilot skills
~/.gemini/skills/         legacy Gemini runtime dir (cleaned up by setup)
~/.agents/skills/         shared Gemini + Codex skills (VS Code should ignore this path)
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

For model routing, copy [model-roles.example.md](model-roles.example.md) to `model-roles.local.md` and customize it.

## Important Docs

| Path | Purpose |
| --- | --- |
| [docs/ai-agent-onboarding.md](docs/ai-agent-onboarding.md) | Reading order for AI agents and maintainers |
| [docs/gal-control-plane-contracts.md](docs/gal-control-plane-contracts.md) | Canonical `/gal` read/write contracts |
| [docs/gstack-integration.md](docs/gstack-integration.md) | Why GAL reimplements gstack semantics natively |
| [docs/gstack-command-contracts.md](docs/gstack-command-contracts.md) | Implementation blueprint for specialist skills |
| [docs/runtime-verification.md](docs/runtime-verification.md) | Live/manual verification status for commands and execution-plane behavior |
| [docs/command-dispatch-architecture.md](docs/command-dispatch-architecture.md) | Dispatch model and alias policy |
| [commands/commands.md](commands/commands.md) | Installed command surface and alias architecture |
| [workflows/coding.md](workflows/coding.md) | Original coding workflow state machine reference |

## What GAL No Longer Treats As Public Workflow

- T0/T1/T2 are not the primary user-facing workflow vocabulary for the new command surface
- Upstream gstack installation is not required to use GAL's specialist commands

## License

MIT — see [LICENSE](LICENSE).
