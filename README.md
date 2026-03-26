# dotdev

A portable, tool-agnostic development methodology system.

Your workflow lives in Markdown. AI tool configs are auto-generated.

## Why

AI coding agents change fast. Your skills and workflow shouldn't be locked into
any single tool (Copilot, Claude Code, Cursor, Gemini CLI, OmO, etc.).

dotdev solves this by separating **what you know** from **which tool reads it**:

- **Knowledge** (workflow, conventions, skills) → Markdown files in this repo
- **Tool configs** (copilot-instructions.md, GEMINI.md, CLAUDE.md) → auto-generated adapters

When you switch tools, you update one routing table and re-run a sync script.
Your knowledge doesn't move.

## Architecture

```text
┌───────────────────────────────────────────────────┐
│ Layer 1: ~/dotdev/  (this repo — your knowledge)  │
│  ├── workflow.md       ← development state machine│
│  ├── model-roles.md    ← model routing table      │
│  ├── conventions/      ← portable language rules  │
│  ├── templates/        ← phase prompt templates   │
│  ├── copilot-skills/   ← skills canonical source  │
│  └── scripts/          ← setup + sync scripts     │
├───────────────────────────────────────────────────┤
│ Layer 2: <repo>/.dev/  (per-repo, tracked in each │
│                         project's own git repo)   │
│  ├── project.md        ← project context          │
│  └── state.md          ← cross-session state      │
├───────────────────────────────────────────────────┤
│ Layer 3: Auto-generated adapters (disposable)     │
│  ├── .github/copilot-instructions.md              │
│  ├── GEMINI.md                                    │
│  ├── CLAUDE.md                                    │
│  └── AGENTS.md                                    │
└───────────────────────────────────────────────────┘
```

## Quick Start

```powershell
# Windows
git clone https://github.com/speedypard/dotdev.git ~/dotdev
~/dotdev/scripts/Setup-Machine.ps1
```

```bash
# macOS
git clone https://github.com/speedypard/dotdev.git ~/dotdev
~/dotdev/scripts/setup-machine.sh
```

Setup scripts create symlinks from `copilot-skills/` → `~/.copilot/skills/`,
so both machines share the same skill definitions.

## Cross-Machine Setup

```text
┌──────────────────────────┐    ┌──────────────────────────┐
│ Windows PC               │    │ Mac Mini                 │
│ ├─ VS Code + Copilot    │    │ ├─ VS Code + Copilot    │
│ ├─ Gemini CLI            │    │ ├─ Xcode (iOS)          │
│ ├─ Ollama (3060 Ti)     │    │ ├─ Ollama (Apple Si)    │
│ └─ Primary dev machine   │    │ └─ Plan Agent (OpenClaw)│
│                          │    │                          │
│ ~/dotdev/ → clone        │    │ ~/dotdev/ → clone        │
└──────────┬───────────────┘    └──────────┬───────────────┘
           └──── git push/pull ────────────┘
```

## Key Files

| File | Purpose |
| :--- | :--- |
| [workflow.md](workflow.md) | Development state machine (PLAN → IMPLEMENT → TEST → REVIEW → VERIFY) |
| [model-roles.md](model-roles.md) | Which AI model plays which role — update this when switching tools |
| `conventions/` | Language-specific rules (portable subset of skills) |
| `templates/` | Prompt templates for each workflow phase |
| `copilot-skills/` | GitHub Copilot skills (canonical source, symlinked to ~/.copilot/skills/) |
| `scripts/` | Machine setup and adapter sync scripts |

## Design Principles

1. **Knowledge in Markdown, not code** — Markdown doesn't have breaking changes
2. **Adapters are disposable** — copilot-instructions.md, GEMINI.md are auto-generated; delete and regenerate anytime
3. **Methodology > Tools** — Tools can be swapped; your workflow stays
4. **GSD / OmO as optional accelerators** — Install them when useful, your methodology doesn't depend on them

## Influences

Concepts borrowed from (implementations not used):

- [GSD](https://github.com/gsd-build/get-shit-done) — Phase-based workflow, STATE.md, verification gates
- [OmO](https://github.com/code-yeongyu/oh-my-openagent) — Category-based model routing (role, not model)
- [LangGraph](https://github.com/langchain-ai/langgraph) — Stateful workflow with persistence and checkpoints
- [OpenClaw](https://github.com/openclaw/openclaw) — Always-on personal agent with sandbox restrictions

## License

Private repository. Consider MIT if publishing.
