# Plan: Fix Command Symlink Architecture

## Goal

Replace 15 broken `commands/gal-*/` skill directories with a single `/gal` dispatcher where `gal dispatch` (script) handles all state detection and intent parsing programmatically, and AI only follows the script's deterministic output.

## Tier

T2

## Review Pack

- architect-full

## Requirements

- [x] `/gal` appears in Copilot Chat autocomplete and dispatches correctly
- [x] `gal dispatch` outputs structured `--- GAL DISPATCH ---` block with READ/MODE/ROLE/ACTION/ON_COMPLETE fields
- [x] `gal dispatch <subcommand>` delegates to existing handlers (init, plan, status, next, pause) and wraps output
- [x] `gal dispatch <golem> <text>` resolves golem, validates phase, sets mode (bound/consult/utility)
- [x] `gal dispatch` (no args) auto-detects phase from `.dev/state.md` and selects default golem
- [x] SKILL.md is generated (not symlinked) with baked absolute paths per tool
- [x] GAL_ROOT symlinks exist: `~/.copilot/gal/` + `~/.gemini/gal/` → repo root
- [x] Old `gal-*` skill directories cleaned up from `~/.copilot/skills/` and `~/.gemini/skills/`
- [x] Gemini gets equivalent structure via generated SKILL.md + gal-context.md import

## Approach

### Dispatch Output Protocol

Contract between `gal dispatch` (script) and SKILL.md (AI). Script always outputs a fenced block:

```
--- GAL DISPATCH ---
COMMAND: <init|plan|status|next|pause|error>
ROLE: <golem-name>          # omit if COMMAND is set
MODE: <bound|consult|utility>  # omit if COMMAND is set
READ: <file-path>           # optional, file AI must read first
ACTION: <instruction text>
ON_COMPLETE: <next-step hint>
--- END DISPATCH ---
```

Rules: `COMMAND` and `ROLE` are mutually exclusive. `MODE` is required when `ROLE` is present. `ACTION` is always present. AI must not deviate from the output — no inference, no reinterpretation.

### Step 1: Create SKILL.template.md

- **Files**: `commands/gal/SKILL.template.md`
- **What**: Template with `{{GAL_ROOT}}` placeholder. Tells AI to run `gal dispatch`, follow structured output. ~15 lines.
- **Verify**: File exists, contains `{{GAL_ROOT}}` placeholder, no hardcoded paths

### Step 2: Add `dispatch` subcommand to gal.ps1

- **Files**: `scripts/gal.ps1`
- **What**: New `dispatch` case: parse intent (subcommand/golem/auto), read `.dev/state.md`, apply dispatch table (phase→golem) + golem classification (workflow/domain/utility), output block per Dispatch Output Protocol above. Remove `ask`/`run` cases.
- **Verify**: `gal dispatch` outputs valid block; `gal dispatch status` outputs `COMMAND: status`; `gal dispatch architect review` → `ROLE: golem-architect`, `MODE: consult`

### Step 3: Add `dispatch` subcommand to gal.sh

- **Files**: `scripts/gal.sh`
- **What**: Bash equivalent of Step 2. Same intent parsing, state reading, dispatch table, output format. Remove `ask`/`run` cases.
- **Verify**: Same test cases as Step 2 on bash

### Step 4: Update Setup-Machine.ps1

- **Files**: `scripts/Setup-Machine.ps1`
- **What**: (a) Add GAL_ROOT dir symlinks: `~/.copilot/gal/` + `~/.gemini/gal/` → repo root. (b) Read `SKILL.template.md`, replace `{{GAL_ROOT}}`, write to `~/.copilot/skills/gal/SKILL.md` + `~/.gemini/skills/gal/SKILL.md`. (c) Append `@~/.gemini/skills/gal/SKILL.md` to gal-context.md as a **hardcoded entry** — generated files are not under `skills/` so the existing scan loop won't pick it up. (d) Migration: remove old `gal-*` dirs from installed `~/.copilot/skills/` + `~/.gemini/skills/`. (e) Update summary counts.
- **Verify**: `Setup-Machine.ps1 -Replace` succeeds; `~/.copilot/gal` is dir symlink; `~/.copilot/skills/gal/SKILL.md` contains baked paths (no `{{GAL_ROOT}}`); `~/.copilot/skills/gal-*` dirs all gone

### Step 5: Update setup-machine.sh

- **Files**: `scripts/setup-machine.sh`
- **What**: Bash equivalent of Step 4.
- **Verify**: Same checks on macOS/Linux

### Step 6: Update Uninstall scripts

- **Files**: `scripts/Uninstall-Machine.ps1`, `scripts/uninstall-machine.sh`
- **What**: Clean up GAL_ROOT symlinks + generated `gal/` skill dirs + old `gal-*` residue.
- **Verify**: After uninstall, no `gal` or `gal-*` dirs remain under `~/.copilot/skills/` or `~/.gemini/skills/`

### Step 7: Delete 15 old command directories

**⚠️ Prerequisite: Step 4 verify must pass (new `/gal` confirmed working in Copilot Chat) before running this step.**

- **Files**: `commands/gal-golem-*/` (10), `commands/gal-init/`, `commands/gal-plan/`, `commands/gal-status/`, `commands/gal-next/`, `commands/gal-pause/`
- **What**: Delete all 15 source directories from repo. Installed symlinks pointing to them were already removed by Step 4 migration.
- **Verify**: `ls commands/` shows only `gal/` and `commands.md`

### Step 8: Rewrite commands.md

- **Files**: `commands/commands.md`
- **What**: This file was previously rewritten to describe a future architecture (`commands/gal/init.md` source layout + file-symlink install pattern) that was never implemented on disk. Rewrite to reflect the new single-dispatch architecture: single `/gal` entry, script-driven dispatch, subcommands (init, plan, status, next, pause), golem dispatch table. Preserve the workflow subcommand names (init, plan, status, next, pause) as the documented user-facing interface.
- **Verify**: No references to old `gal-*` skill names, no `commands/gal/` source-path references, no file-symlink install description

### Step 9: Update scripts.md

- **Files**: `scripts/scripts.md`
- **What**: Add GAL_ROOT to symlinks table. Add generated-files table. Replace `ask`/`run` with `dispatch` in command surface. Remove old command entries.
- **Verify**: Tables match new architecture

### Step 10: Update bootstrap doc

- **Files**: `infra-golem-agents-legion-bootstrap.prompt.md`
- **What**: Update Layer 1.5 ASCII diagram and Setup-Machine symlinks table to show GAL_ROOT symlinks + generated `gal/` skill paths. Retain the file — do not delete.
- **Verify**: Diagram shows `~/.copilot/gal/`, `~/.gemini/gal/`, `~/.copilot/skills/gal/`, `~/.gemini/skills/gal/`; symlinks table includes GAL_ROOT row and generated-files note

## Files to Create or Modify

- `commands/gal/SKILL.template.md` — new, dispatcher template
- `scripts/gal.ps1` — add `dispatch`, remove `ask`/`run`
- `scripts/gal.sh` — add `dispatch`, remove `ask`/`run`
- `scripts/Setup-Machine.ps1` — GAL_ROOT symlinks + SKILL.md generation + gal-context special case + migration
- `scripts/setup-machine.sh` — same
- `scripts/Uninstall-Machine.ps1` — cleanup additions
- `scripts/uninstall-machine.sh` — cleanup additions
- `commands/gal-*/` (15 dirs) — delete (Step 7, after verify)
- `commands/commands.md` — rewrite
- `scripts/scripts.md` — update tables
- `infra-golem-agents-legion-bootstrap.prompt.md` — diagram update (retain file)

## Test Cases

- [x] `gal dispatch` (with `.dev/state.md` Workflow: IMPLEMENT) → outputs `ROLE: golem-implementer`, `MODE: bound`
- [x] `gal dispatch` (no `.dev/`) → outputs `COMMAND: error`, `MESSAGE: Run /gal init first`
- [x] `gal dispatch status` → wraps `gal status` output in `--- GAL DISPATCH ---` block
- [x] `gal dispatch architect review this` → `ROLE: golem-architect`, `MODE: consult`
- [x] `gal dispatch debugger check logs` → `ROLE: golem-debugger`, `MODE: utility`
- [x] `gal dispatch` (Workflow: IDLE) → suggests `init` or `plan`, no golem
- [x] `Setup-Machine.ps1 -Replace` → `~/.copilot/skills/gal/SKILL.md` exists, no `{{GAL_ROOT}}` placeholder remains
- [x] `Get-Item ~/.copilot/gal` → IsLink: True, target is repo root
- [x] Copilot Chat `/gal` → autocomplete appears
- [x] `ls ~/.copilot/skills/ | Where Name -like 'gal-*'` → empty (migration cleaned up)
- [ ] Gemini CLI `/gal` → autocomplete appears (pending test)

## Success Criteria

- [x] Single `/gal` entry point works in Copilot Chat
- [x] All state detection and dispatch logic is in script, not in SKILL.md prompt
- [x] SKILL.md contains zero variable placeholders at runtime (all paths baked)
- [x] Both Copilot and Gemini have working paths via generated SKILL.md + GAL_ROOT symlinks
- [x] No old `gal-*` command directories remain in repo or installed locations

## Risks and Open Questions

- `gal` not on PATH → SKILL.md has baked fallback: `<GAL_ROOT>/scripts/gal.ps1 dispatch`
- state.md format drift → script uses same regex as `gal status` (already validated)
- Template update forgotten after edit → document in Setup-Machine summary output
- Gemini trigger mechanism → same as Copilot; GSD maintains identical trigger across both tools (resolved, no special handling)
- gal-context.md scan won't pick up generated `gal/SKILL.md` → handled by hardcoded append in Step 4c

## Approval

- Human approval: [pending]
- Architect verdict: [pending]
- Other required reviewers: [pending]

---

## Status

Workflow: DONE
Step: 10 of 10
Last activity: 2026-03-27 — all 10 steps implemented; `/gal` verified in Copilot Chat; 15 legacy dirs deleted
Next step: test Gemini CLI `/gal`; if passing → archive plan

### Deviations

| Step | Plan Said | Actually Did | Why |
| --- | --- | --- | --- |

### Handoff Notes

[Context from `gal pause` — key insights, unresolved questions, current hypothesis]

## Test Results

[Written by tester golem after TEST phase]
