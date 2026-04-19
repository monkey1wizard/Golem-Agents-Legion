# Commands

GAL slash commands use a primary `/gal` control-plane entry point plus `gal-*` aliases for autocomplete discoverability.

See [docs/command-dispatch-architecture.md](../docs/command-dispatch-architecture.md) for the architectural rationale and dispatch contract.
See [docs/gal-control-plane-contracts.md](../docs/gal-control-plane-contracts.md) for the current command definitions and read/write contracts.

## Architecture

Command skills are installed into Copilot skill directories and Codex skill directories, while Gemini native slash commands are generated into `~/.gemini/commands/` via `Setup-Machine`. Shared reusable non-command skills remain in `.agents/skills`.

Codex note: installed GAL skills are available as Codex skills, but Codex explicit invocation uses `/skills` or `$skill-name`, not custom `/slash-command` syntax. Example: use `$gal status`, not `/gal status`.

**Four runtime layers:**

| Layer | Path | Content |
| --- | --- | --- |
| Installed dispatcher entry | `~/.copilot/skills/gal/SKILL.md`, `~/.gemini/commands/gal.toml`, `~/.codex/skills/gal/SKILL.md` | Main entry point — generated from `commands/gal/SKILL.template.md` |
| Discoverability aliases | `~/.copilot/skills/gal-*/SKILL.md`, `~/.gemini/commands/gal-*.toml`, `~/.codex/skills/gal-*/SKILL.md` | Alias entries — generated from `commands/gal-*/SKILL.template.md` |
| GAL references | `~/.copilot/gal/`, `~/.gemini/gal/` → repo symlink | templates/, workflows/, conventions/, agent/ |
| Workspace context | `.dev/project.md`, `.dev/state.md` | Per-repo state |

**Key principles:**

- **Primary control-plane entry** — `/gal` is the main interface. It routes subcommands, consults golems, and handles state-aware dispatch.
- **Substantive alias skills** — `/gal-status`, `/gal-whats-next`, and `/gal-wrap-up` contain full procedures and do not dispatch through the script.
- **Script-dispatched subcommands** — `init` and `research` still route through `gal.ps1 dispatch`.
- **Discoverability aliases** — `/gal-init`, `/gal-status`, `/gal-whats-next`, `/gal-wrap-up` exist so typing `/gal-` exposes controls in slash-command autocomplete.
- **Baked absolute paths** — `Setup-Machine.ps1` and `setup-machine.sh` replace `{{GAL_ROOT}}` with the absolute repo path before installation.
- **Runtime-agnostic procedures** — shared command templates refer to installed skill names rather than a Copilot-only path.
- **One repo symlink** — `~/.copilot/gal/` and `~/.gemini/gal/` point to the GAL repo root.
- **Split command surfaces** — Gemini gets native slash commands from `~/.gemini/commands/`; Codex gets named command skills from `~/.codex/skills/`; shared reusable skills stay in `.agents/skills`.

## Command Surface

### `/gal` — Control Plane Entry Point

```sh
/gal [subcommand | golem-name | free text]
```

| Invocation | What It Answers | Behaviour |
| --- | --- | --- |
| `/gal` | What should I do? | Auto-detect from `.dev/state.md` — follow `/gal-whats-next` procedure |
| `/gal init` | How do I bootstrap this repo? | Scaffold `.dev/project.md` + `.dev/state.md` via script |
| `/gal status` | Where are we right now? | Full state projection — active plans, review/test/blockers/continuity |
| `/gal whats-next` | What do I do next? | Read state and recommend single next action or command |
| `/gal wrap-up` | How do I close this session? | Converge handoff updates and update session continuity |
| `/gal research` | I need structured investigation | Invoke research golem via script |
| `/gal <golem-name>` | Consult a specific golem | Invoke that golem via script |

### `gal-*` — Discoverability Aliases

These aliases exist for slash-command autocomplete discoverability in command surfaces that support custom slash commands.

| Alias | Status | Purpose |
| --- | --- | --- |
| `/gal-init` | Active | Bootstrap `.dev/` for a repo |
| `/gal-status` | Active | Full state projection (substantive skill, no script) |
| `/gal-whats-next` | Active | Next-action recommendation (substantive skill, no script) |
| `/gal-wrap-up` | Active | Session close-out (substantive skill, no script) |

For Codex CLI, use the equivalent skill names with `$` invocation:

| Codex Skill | Equivalent GAL Command |
| --- | --- |
| `$gal` | `/gal ...` |
| `$gal-init` | `/gal init` |
| `$gal-status` | `/gal status` |
| `$gal-whats-next` | `/gal whats-next` |
| `$gal-wrap-up` | `/gal wrap-up` |

### Dispatch Output Protocol

For script-dispatched subcommands (`init`, `research`, golem names), the CLI emits a block that the AI reads and executes:

```text
--- GAL DISPATCH ---
COMMAND: <init|error|suggest>
ROLE: <golem-name>           # mutually exclusive with COMMAND
MODE: <bound|consult|utility>  # required when ROLE present
READ: <file-path>            # optional, may appear multiple times
ACTION: <instruction text>
ON_COMPLETE: <next-step hint>
--- END DISPATCH ---
```

### Golem Classification

| Golem | Class | Default Mode |
| --- | --- | --- |
| golem-implementer | Pipeline | consult |
| golem-tester | Pipeline | consult |
| golem-reviewer | Pipeline | consult |
| golem-verifier | Pipeline | consult |
| golem-architect | Domain | consult |
| golem-analyst | Domain | consult |
| golem-designer | Domain | consult |
| golem-researcher | Domain | consult |
| golem-librarian | Domain | consult |
| golem-debugger | Utility | utility |
| golem-scribe | Utility | utility |

## Planning-Stage Review Lanes

GAL keeps planning as a native command family and routes planning-stage review by lane, not by legacy command name.

| Lane | Preferred Provider | Fallback Invocation | Write-Back Target |
| --- | --- | --- | --- |
| Business / Scope review | Upstream gstack business review provider when installed | `/gal golem-analyst` | `## Review Results` + `## Open Questions` |
| Design review | Upstream gstack design review provider when installed | `/gal golem-designer` | `## Review Results` + `## Open Questions` |
| Engineering review | Upstream gstack engineering review provider when installed | `/gal golem-architect` | `## Review Results` + `## Open Questions` + `## Test Plan` + `## Tasks` + `<!-- ENG_REVIEW: CLEAR -->` |

Legacy gstack planning command names are not part of GAL's public command catalog. They survive only as provider semantics in integration contracts.

## Design Execution

These commands shape or implement design work before or during implementation. They are not the same as planning-stage design review lanes.

| Command | Purpose | Phase |
| --- | --- | --- |
| `/design-consultation` | Establish or refine the repo-level design system in `DESIGN.md` | Project setup / design preparation |
| `/design-shotgun` | Explore multiple visual variants before code is written | Design exploration |
| `/design-html` | Convert an approved mockup into HTML or a framework component | Design engineering / implementation |

## Post-Implementation Review And Audit

These commands belong to the workflow review stage after code exists.

| Command | Purpose | Phase |
| --- | --- | --- |
| `/review` | Paranoid staff code review for correctness, completeness, and drift | Post-implementation review |
| `/design-review` | Live UI audit against `DESIGN.md` for customer-facing changes | Post-implementation review |
| `/cso` | OWASP and STRIDE security audit for auth, data, input, or public API changes | Post-implementation review |
| `/qa`, `/qa-only` | Execute the test plan and verify behavior in the browser | Review / test stage |

`/review` and `/design-review` are both workflow review-stage specialists. One audits the code changes; the other audits the running product experience.

## Session Safety

These commands control risk for the current session. They are not planning lanes or review commands.

| Command | Purpose | When to Prefer |
| --- | --- | --- |
| `/careful` | Warn before destructive commands | General risky work |
| `/freeze` | Lock edits to a directory boundary | Focused refactors and debugging |
| `/guard` | Enable `/careful` and `/freeze` together | Production systems, live data, shared risky config |
| `/unfreeze` | Remove the current freeze boundary | After a bounded debug or refactor session |

## Source Files

| File | Purpose |
| --- | --- |
| `commands/gal/SKILL.template.md` | Dispatcher source template |
| `commands/gal-init/SKILL.template.md` | Alias template for `/gal-init` |
| `commands/gal-status/SKILL.template.md` | Alias template for `/gal-status` |
| `commands/gal-whats-next/SKILL.template.md` | Alias template for `/gal-whats-next` |
| `commands/gal-wrap-up/SKILL.template.md` | Alias template for `/gal-wrap-up` |
| `commands/<specialist>/SKILL.template.md` | Specialist command templates (one per command) |

## Installation

Managed by `Setup-Machine.ps1` / `setup-machine.sh`. The scripts:

1. Create `~/.copilot/gal/` and `~/.gemini/gal/` → GAL repo root symlinks.
2. Scan all `commands/*/SKILL.template.md` files, replacing `{{GAL_ROOT}}` with the absolute path.
3. Write baked `SKILL.md` files into each `commands/*/` directory.
4. Symlink all command directories into `~/.copilot/skills/` and `~/.codex/skills/`.
5. Generate Gemini native command files in `~/.gemini/commands/` from the baked `SKILL.md` content.
