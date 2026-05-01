# Research Brief: Caveman ↔ GAL Compatibility

**Date:** 2026-04-30
**Researcher:** golem-researcher (GitHub Copilot session)
**Method:** local-first vault search → external fetch → live machine probe → subagent verification

---

## Research Question

Can `juliusbrussee/caveman` (output-token compression skill) be integrated with GAL without breaking GAL's generated-adapter model? What is the current machine state, and what are the file collision risks?

---

## Raw Findings

### What Caveman Is

- Caveman v1.6.0 — output-token compression framework; claims ~75% token reduction on responses — [README](https://github.com/JuliusBrussee/caveman/blob/main/README.md)
- Pure prompt/skill layer; no model dependency — [README](https://github.com/JuliusBrussee/caveman/blob/main/README.md)
- MIT licensed — [README](https://github.com/JuliusBrussee/caveman/blob/main/README.md)
- Two activation modes: `ultra` (max compression) and `natural` (readable prose) — [README](https://github.com/JuliusBrussee/caveman/blob/main/README.md)
- Install via `npx caveman` (global skill installer) or `npx skills add -a <runtime>` (runtime-targeted) — [README](https://github.com/JuliusBrussee/caveman/blob/main/README.md)

### Runtime Support Matrix

| Runtime | Integration Type | Files Written | Hook System |
| --- | --- | --- | --- |
| Claude Code | Plugin + hooks | `~/.claude/CLAUDE.md`, hooks in `~/.claude/hooks/` | Full (SessionStart, UserPromptSubmit, statusline) |
| Gemini CLI | Extension | `GEMINI.md` (repo-local), `~/.gemini/settings.json` | Via extension mechanism |
| GitHub Copilot | Skill-only | `.github/copilot-instructions.md`, `AGENTS.md` | None — static instruction surface only |
| Codex CLI | Via `npx skills add` | `AGENTS.md` | No dedicated hook system |

Sources: [caveman/skills/](https://github.com/JuliusBrussee/caveman/tree/main/skills), [caveman/hooks/README.md](https://raw.githubusercontent.com/JuliusBrussee/caveman/main/hooks/README.md), [caveman/.github/copilot-instructions.md](https://raw.githubusercontent.com/JuliusBrussee/caveman/main/.github/copilot-instructions.md)

### Claude Code Hook Architecture

Caveman registers three hooks for Claude Code:

- **SessionStart** — writes `~/.claude/.caveman-active` flag file; injects mode into Claude context
- **UserPromptSubmit** — detects `/ultra` or `/natural` triggers in user input; updates flag file; passes mode context forward
- **Statusline** — reads flag file; displays `🦴` or `📝` badge in terminal statusline

Source: [hooks/README.md](https://raw.githubusercontent.com/JuliusBrussee/caveman/main/hooks/README.md)

### Copilot Static Skill Content

`npx skills add -a github-copilot` writes a static prompt snippet to `.github/copilot-instructions.md` and `AGENTS.md`. Content is a small block of terse rules (compress output, prefer abbreviations, etc.). No hook registration. Mode switching requires manual prompt prefixes only.

Source: [caveman/.github/copilot-instructions.md](https://raw.githubusercontent.com/JuliusBrussee/caveman/main/.github/copilot-instructions.md)

### Local Machine State (Windows, 2026-04-30)

- **Gemini CLI:** caveman extension v1.6.0 installed. `~/.gemini/settings.json` contains `caveman` entry under `extensions`. Functional.
- **GitHub Copilot:** No caveman skill installed in `~/.copilot/skills/` or any VS Code extension path.
- **`~/.agents/skills/`:** `caveman*` directories present — conflict risk with Gemini extension's own skill path.

---

## Synthesis

### What Appears True

1. **Caveman works on Gemini CLI today** — extension installed, GEMINI.md route active, no GAL collision observed yet (GAL-generated GEMINI.md carries `Do not edit manually`; conflict is latent, not yet surfaced).
2. **Caveman is not installed on Copilot today** — no skill in any Copilot path.
3. **Claude Code has the richest integration** — hook system enables automatic mode persistence across prompts, statusline badge, and session-level state. This is a capability gap, not a feature choice: Copilot has no equivalent hook surface.
4. **GAL-generated files are the core collision risk** — caveman targets the same files GAL regenerates:
   - `.github/copilot-instructions.md` → GAL: generated from source docs
   - `AGENTS.md` → GAL: generated from source docs
   - `GEMINI.md` → GAL: generated from source docs
   - `CLAUDE.md` → GAL: generated from source docs (Claude route)
5. **Post-regeneration overwrites caveman edits** — any `npx skills add` writes into generated outputs will be erased the next time `Sync-DevContext.ps1` runs.

### Why Claude Code Has Full Support vs Copilot

The distinction is architectural:

- Claude Code exposes a **hook system** — external processes registered in `settings.json` that fire on lifecycle events (session start, prompt submit). Caveman exploits these hooks to maintain persistent state (`~/.claude/.caveman-active`) and re-inject mode on every prompt without user action.
- GitHub Copilot exposes only a **static instruction surface** — a Markdown file read at context-load time. There are no lifecycle events, no hook registration points, no external process execution. Mode changes require the user to type a prefix on every relevant prompt.

This gap is a Copilot platform constraint, not something caveman or GAL can bridge without a Copilot-side hook API.

### What Is Uncertain

- Whether `~/.agents/skills/caveman*` duplicates the Gemini extension state or creates conflicts.
- Whether caveman's Gemini extension reads `GEMINI.md` from the repo root at session start, which would mean GAL's generated content is already competing.
- Actual token savings in practice on GAL-style workflow prompts (claimed ~75% is benchmark-reported, not measured locally).

### Integration Path Options

| Option | Description | Risk |
| --- | --- | --- |
| **A: Embed caveman rules in GAL source inputs** | Add caveman terse-output rules to a `SKILL.local.md` overlay or a new `skills/caveman/SKILL.md`; Sync-DevContext propagates on next run | Low — survives regeneration; follows existing personalization model |
| **B: Post-regeneration injection script** | Patch generated adapters after Sync-DevContext; script appends caveman block | Medium — fragile ordering; must run after every sync |
| **C: Use caveman for Claude Code only via hooks** | No GAL file changes; caveman hook system handles Claude Code natively; Copilot operates without caveman | Zero file collision risk; leaves Copilot uncovered |
| **D: Do nothing** | Gemini already has it; Copilot doesn't need it | Latent GEMINI.md collision remains unresolved |

**Recommended path: Option A** — embed caveman terse-output rules in a GAL-owned skill or SKILL.local.md overlay so rules survive regeneration and Sync-DevContext distributes them through the existing adapter pipeline.

---

## Reference Verification

All references below were fetched live during this session (2026-04-30).

| Reference | Verifier Outcome |
| --- | --- |
| `https://github.com/JuliusBrussee/caveman/blob/main/README.md` | Verified — version 1.6.0, install commands, mode descriptions confirmed |
| `https://raw.githubusercontent.com/JuliusBrussee/caveman/main/hooks/README.md` | Verified — three hooks (SessionStart, UserPromptSubmit, statusline) with flag file path `~/.claude/.caveman-active` |
| `https://raw.githubusercontent.com/JuliusBrussee/caveman/main/.github/copilot-instructions.md` | Verified — static prompt snippet only; no hook references |
| `https://raw.githubusercontent.com/JuliusBrussee/caveman/main/skills/` | Partially verified — directory listing confirmed; individual skill files fetched |
| Local machine state — Gemini caveman v1.6.0 | Verified via `~/.gemini/settings.json` and `Get-ChildItem ~/.agents/skills/` |
| Local machine state — Copilot no caveman | Verified via `~/.copilot/skills/` enumeration |

> **Note:** VERIFY state requires a different model from the research author. The above is self-verification only. An independent model should reverse-check the hook file contents and the machine state claims before this brief is treated as settled.

---

## Gaps / Open Questions

- `~/.agents/skills/caveman*` origin unclear — is this caveman's own runtime skill store, Gemini extension side-effect, or a third path?
- Does caveman's Gemini extension inject into `GEMINI.md` at install time or only at session time? If install-time, GAL's GEMINI.md is already partially colliding.
- Caveman token-reduction claims (~75%) are not measured locally on GAL-style prompts. Actual savings on long instruction files (like copilot-instructions.md) may differ significantly.
- No Windows-native caveman hook path tested — hooks documentation assumes `~/.claude/` paths; Windows `%USERPROFILE%` behavior not confirmed.

---

## Recommended Next Step

- **If pursuing integration:** Implement Option A — create `skills/caveman/SKILL.md` with caveman terse-output rules following the existing skill pattern. Then re-run `Sync-DevContext.ps1` to propagate. No generated-adapter edits required.
- **If not pursuing integration:** Close the latent GEMINI.md collision by documenting that Gemini's caveman extension is a machine-local opt-in that will be overwritten by the next Sync-DevContext run.
- **Independent VERIFY:** A second model should confirm the hook lifecycle claims from `hooks/README.md` and validate that the Copilot static-surface limitation is not a documentation gap but a platform-level absence.
