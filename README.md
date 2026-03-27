# Golem Agents Legion (GAL)

English | [繁體中文](README.zh-Hant.md)

A portable, tool-agnostic development methodology system.

> **Methodology is written in Markdown, tools are auto-generated.**
> You own the methodology in `golem-agents-legion/`, not any tool's config.
> Tools come and go. Your knowledge stays.

## Why

AI coding agents change fast. Your skills and workflow shouldn't be locked into
any single tool (Copilot, Claude Code, Cursor, Gemini CLI, etc.).

GAL solves this by separating **what you know** from **which tool reads it**:

- **Knowledge** (workflows, conventions, agents, skills) → Markdown files in this repo
- **Tool configs** (`copilot-instructions.md`, `GEMINI.md`, `CLAUDE.md`) → auto-generated adapters

When you switch tools, update one routing table and re-run a sync script. Your knowledge doesn't move.

## Three-Layer Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│ Layer 1: ~/golem-agents-legion/  (this repo — your brain)   │
│  ├── workflows/         ← state machines (coding, research) │
│  ├── agent/             ← 9 golem agent definitions         │
│  ├── model-roles.md     ← model routing + tier system       │
│  ├── conventions/       ← portable language rules           │
│  ├── templates/         ← plan, state, project scaffolds    │
│  ├── skills/            ← canonical skill source (symlinked)│
│  └── scripts/           ← setup, init, sync, dispatcher     │
├─────────────────────────────────────────────────────────────┤
│ Layer 2: <repo>/.dev/   (per-repo, tracked in each project) │
│  ├── project.md         ← project summary + index           │
│  └── state.md           ← active plans + session continuity │
├─────────────────────────────────────────────────────────────┤
│ Layer 3: Auto-generated adapters (disposable)               │
│  ├── .github/copilot-instructions.md                        │
│  ├── GEMINI.md                                              │
│  ├── CLAUDE.md                                              │
│  └── AGENTS.md                                              │
└─────────────────────────────────────────────────────────────┘
```

## 9 Golem Agents

| Agent | Classification | Purpose |
| --- | --- | --- |
| **planner** | Workflow | Analyze requirements, produce plan files |
| **architect** | Domain | Adversarial plan review — trade-offs, over-engineering, bugs |
| **analyst** | Domain | Business logic review — ROI, domain correctness |
| **implementer** | Workflow | Execute plans with atomic commits + Scope Fence |
| **tester** | Workflow | Write tests from spec only (never reads implementation) |
| **reviewer** | Workflow | Cross-review for bugs, security, architecture |
| **verifier** | Workflow | Goal-backward verification + plan lifecycle ending |
| **debugger** | Utility | Scientific method bug investigation |
| **scribe** | Utility | End-of-day diary + curfew enforcer |

## Coding Flow Tiers

| Tier | When | Process |
| --- | --- | --- |
| **T0** (Trivial) | Typo, obvious bug, single file | IMPLEMENT → DONE |
| **T1** (Standard) | Small feature, known-cause fix | PLAN → IMPLEMENT → TEST → REVIEW(lite) → VERIFY |
| **T2** (Strategic) | New feature, arch change, high risk | PLAN → DISCUSS → APPROVE → IMPLEMENT → TEST → REVIEW → VERIFY |

See [workflows/coding.md](workflows/coding.md) for the complete state machine.

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

Setup scripts create symlinks: `agent/` → `~/.copilot/agents/`, `skills/` → `~/.copilot/skills/`.
Both machines share the same definitions via `git push/pull`.

## Cross-Machine Setup

GAL supports multi-machine workflows. Each machine clones the same repo and
runs `Setup-Machine` to create symlinks. Models and tools differ per machine;
the methodology stays identical.

```text
┌────────────────────────────┐    ┌────────────────────────────┐
│ Machine A                  │    │ Machine B                  │
│ ├─ AI coding tools         │    │ ├─ AI coding tools         │
│ ├─ Ollama (local models)   │    │ ├─ Ollama (local models)   │
│ └─ Primary dev machine     │    │ └─ Secondary / mobile      │
│                            │    │                            │
│ ~/golem-agents-legion/     │    │ ~/golem-agents-legion/     │
└──────────┬─────────────────┘    └──────────┬─────────────────┘
           └────── git push/pull ────────────┘
```

See [model-roles.md](model-roles.md) for how to map roles to your specific machines and models.

## Personalization

Several files contain `<PLACEHOLDER>` values that you must fill in after cloning:

| Placeholder | Meaning | Files |
| --- | --- | --- |
| `<OBSIDIAN_VAULT>` | Absolute path to your Obsidian vault | `agent/golem-scribe.agent.md`, `skills/obsidian-cli/`, `skills/local-first-search/` |
| `<OBSIDIAN_VAULT_NAME>` | Vault name as shown in Obsidian | `skills/obsidian-cli/` |
| `<LOCAL_SEARCH_PROJECT>` | Path to your `obsidian-note-taking-assistant` clone | `skills/local-first-search/`, `skills/obsidian-knowledge-management/` |
| `<GAL_SKILLS>` | Path where skills are installed (e.g. `~/.copilot/skills`) | `skills/pdf/` |
| `<TEMP_DIR>` | Temporary directory for output | `skills/pdf/` |

For model mapping, copy [`model-roles.example.md`](model-roles.example.md) to `model-roles.local.md` and customize.

## Key Files

| Path | Purpose |
| --- | --- |
| [workflows/coding.md](workflows/coding.md) | Coding Flow state machine (Tier + Scope Fence + review pack) |
| [model-roles.md](model-roles.md) | Model routing + tier system — update when switching tools |
| [agent/](agent/agents.md) | 9 golem agent definitions |
| [conventions/](conventions/conventions.md) | Portable language rules (universal, C#, Go, TS, Rust) |
| [templates/](templates/templates.md) | Plan, state, project, diary, agent scaffolds |
| [skills/](skills/) | Copilot skills (canonical source, symlinked to `~/.copilot/skills/`) |
| [scripts/](scripts/scripts.md) | Machine setup, repo init, workflow dispatcher |
| [ROADMAP.md](ROADMAP.md) | Implementation progress and milestones |

## Design Principles

1. **Knowledge in Markdown, not code** — Markdown doesn't have breaking changes
2. **Adapters are disposable** — `copilot-instructions.md`, `GEMINI.md` are auto-generated; delete and regenerate anytime
3. **Methodology > Tools** — Tools can be swapped; your workflow stays
4. **Plan as transient memory** — Plans are created, executed, knowledge extracted to `docs/`, then deleted
5. **Human is orchestrator** — Golems are specialists; the human decides tier, scope, and when to proceed

## Influences

Concepts borrowed from (implementations not used):

- [GSD](https://github.com/gsd-build/get-shit-done) — Phase-based workflow, state tracking, verification gates
- [OmO](https://github.com/code-yeongyu/oh-my-openagent) — Category-based model routing (role, not model)
- [LangGraph](https://github.com/langchain-ai/langgraph) — Stateful workflow with persistence and checkpoints

## License

MIT — see [LICENSE](LICENSE).
