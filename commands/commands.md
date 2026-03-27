# Commands

GAL slash command — a single `/gal` dispatcher installed as `~/.copilot/skills/gal/SKILL.md`.

## Architecture

A single skill file backed by a baked-path template. The AI discovers it when the user types `/gal` in Copilot Chat or Gemini.

**Three layers at runtime:**

| Layer | Path | Content |
|---|---|---|
| Dispatcher skill | `~/.copilot/skills/gal/SKILL.md` | Entry point — generated from `commands/gal/SKILL.template.md` |
| GAL references | `~/.copilot/gal/` → repo symlink | templates/, workflows/, conventions/, agent/ |
| Workspace context | `.dev/project.md`, `.dev/state.md` | Per-repo state |

**Key principles:**

- **Single entry point** — `/gal` handles all commands. The AI reads the dispatch block from `gal.ps1`/`gal.sh` output and routes accordingly.
- **Script-driven dispatch** — `gal dispatch` detects workflow state, golem intent, or subcommand and emits a structured `--- GAL DISPATCH ---` block. The AI follows it deterministically.
- **GAL_ROOT baked** — `Setup-Machine.ps1` reads `SKILL.template.md`, replaces `{{GAL_ROOT}}` with the absolute repo path, and writes the final `SKILL.md`. No runtime path resolution needed.
- **One repo symlink** — `~/.copilot/gal/` → GAL repo root. All conventions, workflows, and templates accessible.

## Command Surface

### `/gal` — Dispatcher

```
/gal [subcommand | golem-name | free text]
```

| Invocation | Behaviour |
|---|---|
| `/gal` | Auto-detect from `.dev/state.md` → route to bound golem |
| `/gal init` | Initialize `.dev/` for a repo — scaffold `project.md` + `state.md` |
| `/gal plan` | Create plan scaffold from template |
| `/gal status` | Show current workflow state |
| `/gal next` | Show next step for session resumption |
| `/gal pause` | Commit context for session handoff |
| `/gal golem-architect` | Consult architect (domain, consult mode) |
| `/gal golem-debugger` | Invoke debugger (utility, any state) |
| `/gal <any golem name>` | Invoke that golem directly |

### Dispatch Output Protocol

The CLI emits a block that the AI reads and executes:

```
--- GAL DISPATCH ---
COMMAND: <init|plan|status|next|pause|error|suggest>
ROLE: <golem-name>           # mutually exclusive with COMMAND
MODE: <bound|consult|utility>  # required when ROLE present
READ: <file-path>            # optional, may appear multiple times
ACTION: <instruction text>
ON_COMPLETE: <next-step hint>
--- END DISPATCH ---
```

### Golem Classification

| Golem | Class | Bound State | Default Mode |
|---|---|---|---|
| golem-planner | Workflow | PLAN | bound / consult |
| golem-implementer | Workflow | IMPLEMENT | bound / consult |
| golem-tester | Workflow | TEST | bound / consult |
| golem-reviewer | Workflow | CROSS_REVIEW | bound / consult |
| golem-verifier | Workflow | VERIFY | bound / consult |
| golem-architect | Domain | — | consult |
| golem-analyst | Domain | — | consult |
| golem-librarian | Domain | — | consult |
| golem-debugger | Utility | any | utility |
| golem-scribe | Utility | any | utility |

Workflow golems use `bound` mode when the current state matches their bound state, `consult` otherwise.

## Source Files

| File | Purpose |
|---|---|
| `commands/gal/SKILL.template.md` | Template — `{{GAL_ROOT}}` placeholder, baked by Setup-Machine |

## Installation

Managed by `Setup-Machine.ps1` / `setup-machine.sh`. The scripts:

1. Create `~/.copilot/gal/` and `~/.gemini/gal/` → GAL repo root symlinks
2. Read `commands/gal/SKILL.template.md`, replace `{{GAL_ROOT}}` with absolute path
3. Write baked `SKILL.md` to `~/.copilot/skills/gal/` and `~/.gemini/skills/gal/`
4. Remove legacy `gal-*` skill dirs from both targets (migration)
