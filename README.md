# Golem-Agents-Legion

A document-driven AI working system for solo developers who want one stable workflow across Claude Code, Codex CLI, GitHub Copilot, Antigravity CLI and opencode. GAL keeps planning, implementation, testing, review, and research in repo-owned Markdown so work moves between AI tools without losing context.

**The signature is the loop — and it runs across vendors.** Planning converges through a REFINE-LOCK loop; execution runs an autonomous pipeline loop that dispatches each phase to a *different vendor's* coding agent — implement on one, test on another, audit on a third — so every change is checked cross-model instead of by the agent that wrote it.

This README is the map. It links out to the docs that own each topic in depth — it does not repeat them.

## What is GAL

- A small **control-plane** command surface (`/gal …`) plus a set of specialized **golem agents**.
- **Repo-owned state** (`.dev/`, `.dev/plans/`, `.dev/research/`) instead of provider-local chat memory, so any tool can resume where another stopped.
- **Cross-runtime parity**: the same contracts drive every supported AI CLI.

## Why GAL

- **A cross-vendor loop, not a single agent** — `/gal pipeline` loops implement → test → audit → commit per task (then one end-of-run goal-backward verify) and dispatches each implement/test/audit phase to a different vendor's coding agent (e.g. code on Claude, tests on Codex, audit on Copilot). Independent cross-model checking is built into the loop, not bolted on.
- **Plans converge before they execute** — the REFINE-LOCK loop iterates planning through a Definition-of-Ready gate instead of rushing into code.
- **Switch AI tools mid-feature without re-explaining anything** — state lives in the repo, not in one tool's chat history, so any runtime resumes where another stopped.
- **One contract, five runtimes** — Claude Code, Codex CLI, GitHub Copilot, Antigravity CLI, and opencode all follow the same workflow.

## Quick Start

GAL's primary path is **install mode** — you do not need to clone this repo.

1. **Install the `gal` CLI** (install mode; see [Install & Distribution](#install--distribution) for what is shipped today).
2. **In your project**, open a supported runtime and run `/gal init` (Codex uses `$gal init`).
3. **Work the flow**: `/planning` → `/gal pipeline` → `/gal finalize`.

Detailed first-time setup, machine config, and daily operations live in the **[user manual](docs/manual.md)**.

## Working on GAL

To work on GAL itself (contributor setup), use a local clone:

```bash
git clone https://github.com/monkey1wizard/golem-agents-legion.git
# In the repo:     gal init

```

The `gal` binary resolves its source root automatically from the checkout — no mode flag to set. Full dev setup, runtime topology, and the `~/.gal/` layout are in the **[developer guide](docs/devguide.md)**.

## Install & Distribution

Install the `gal` binary via your package manager, then run `gal init` in each repo to generate the adapters:

| Channel | Status |
| --- | --- |
| `cargo install --git` | available now |
| `winget` · `homebrew` · GitHub Releases | future milestone (public release pipeline in `packaging/`) |

### From the Claude / Codex marketplace (two steps)

You can also discover GAL as a plugin and let your AI install the binary:

1. **Find and install the GAL plugin** in the Claude Code or Codex plugin marketplace (search "gal"), or add it from the [marketplace snapshot branch](https://github.com/monkey1wizard/golem-agents-legion/tree/marketplace-snapshot) directly.
2. **Ask the AI to finish setup** — say "help me install gal". The plugin ships an
   `install-gal` skill: with your consent it runs the OS-appropriate package-manager
   command (Homebrew / winget / `cargo install --git`), then verifies `gal --version`.

The plugin alone is not yet a working GAL — every command except `install-gal` needs
the `gal` binary, so step 2 completes the install. Marketplace listing and name-search
make GAL discoverable, but enhanced placement / curated featuring is not guaranteed.

`gal doctor` checks setup health. `gal init --force` regenerates repo-local adapters (CLAUDE.md, AGENTS.md, etc.) from gal-core templates. Machine-local intent that must be preserved across reinstalls: `~/.gal/config/config.json`. The full live-surface contract and acceptance bar live in [developer guide → GAL-Owned Live Read Surfaces](docs/devguide.md#gal-owned-live-read-surfaces-per-provider).

## How GAL Works

A one-glance mental model. Each row links to the doc that owns the full contract.

### Feature lifecycle

```text
/gal init
   ↓
/planning
   ↓
/deep-planning ─┐
   ↓            │ REFINE-LOCK loop
/refining-plan ─┘ refine ⇄ dual-lens DoR ⇄ converge
   ↓
human approval
   ↓
/plan-to-prompt
   ↓
/gal pipeline
   ↓
/gal finalize
```

`/planning` writes a human-readable plan to `.dev/plans/`. `/deep-planning` runs mandatory architect review. `/refining-plan` is not a single step but a **REFINE-LOCK loop**: it drafts `## Tasks` + `## Test Plan`, then iterates through a Definition-of-Ready dual-lens gate (architect + tester) and a steward documentation-structure check at refining-end, looping back on any REVISE until it converges. Human approval must be recorded in `## Approval` before `/plan-to-prompt` produces the execution work file. `/gal pipeline` chains implement → test → audit per task and ends when the goal-backward verify passes (VERIFIED). `/gal finalize` then lands the verified plan — whole-branch holistic review, merge into main, and lifecycle close (release only when the plan actually ships an artifact). (`/gal wrap-up` is a separate session-**pause** job, callable anytime to record continuity mid-flight — it does not land or close anything.) Full loop semantics: [plugins/gal-core/workflows/coding.md](plugins/gal-core/workflows/coding.md#stage-35--definition-of-ready-gate-refine-lock-loop).

### Commands

`/gal init · status · whats-next · wrap-up · research · deep-research · pipeline` plus `/planning · deep-planning · refining-plan · plan-to-prompt`. Full contract: [plugins/gal-core/commands/commands.md](plugins/gal-core/commands/commands.md).

### Golem agents

10 specialized agents in three categories — **Utility** (debugger), **Domain** (architect, analyst, designer [UI/UX + DevEx], researcher, releaser, steward), **Pipeline** (implementer, tester, auditor). Independent tester/auditor (different models) plus orchestrator-owned goal-backward verification keep verification credible. A user-facing capability table (what each does / when to use it) is in [docs/manual.md → Golem Agents](docs/manual.md#golem-agents); the full roster and rules: [plugins/gal-core/agents/agents.md](plugins/gal-core/agents/agents.md) and [plugins/gal-core/workflows/coding.md](plugins/gal-core/workflows/coding.md).

**Consult dual-mode** — planning-stage roles (architect, analyst, designer) support two invocation modes:

| Mode | Trigger | What happens |
| --- | --- | --- |
| **Isolated** (default) | `/gal <role>` | Native subagent runs role in isolation; only verdict/summary returns to main context. Label: `[<role> · isolated]` |
| **In-context** | `/gal discuss <role>` | Activation-core loads into current conversation; hot-joins from prior isolated verdict; continues multi-turn. Label: `[<role> · in-context]` |

**Role invocability** — directly callable: architect, analyst, designer, releaser (consult), debugger, steward. Orchestrated-only (bare call → unknown-intent): implementer, tester, auditor, researcher (available only via `/gal pipeline`, `/gal finalize`, or `/gal research`).

**Checking-role triangle** — three independent owners keep output trustworthy, so no agent grades its own work:

- **ORCHESTRATOR** — per-task correctness gate + end-of-run goal-backward verify + plan lifecycle close
- **AUDITOR** — independent deep performance + security audit, a different model from the implementer
- **STEWARD** — documentation structure: drift detection, structure map, knowledge extraction → durable layer (`README.md` + `docs/` excl. `.dev/plans/` + `.dev/research/`). Runs at planning (plan-document structure only) **and** at finalize (the completed plan's knowledge **must** land in `docs/` — a hard rule gating plan-file deletion). See [docs/manual.md](docs/manual.md#golem-agents) → Steward lifecycle split.

### Pipeline

```text
per task (T-NN):

  ORCHESTRATOR (dispatches each phase, checks each return)
       |
       |dispatch task T-NN
       ↓
  CODER (implement) ◄─────────┐
       ↓                      │
  TESTER (unit test) ──FAIL───┤ re-dispatch
       ↓ pass                 │ implement
  AUDITOR (review) ──REJECT───┘ (fix, ≤3)
       ↓ pass
  ORCHESTRATOR (commit + 3-surface converge)
       ↓
  next task ↺

all tasks done:

  ORCHESTRATOR (goal-backward verify, in-process)
       ↓
  VERIFIED → /gal finalize
```

This is **the loop**: each phase is dispatched to a routed executor (`config.json#executorRouting`), so the implementer, tester, and auditor can each be a **different vendor's coding agent** — independent cross-model verification by construction. The correctness gate is owned by the orchestrator and the deep-performance/security audit is owned by `golem-auditor` on every task. The full workflow semantics live in [plugins/gal-core/workflows/coding.md](plugins/gal-core/workflows/coding.md).

### Research

A workflow parallel to development. `/gal research` (RESEARCH → VERIFY → DOCUMENT) and `/gal deep-research` (adds SYNTHESIZE → CROSS-REVIEW, ≥5 sources). VERIFY must use a different model from the author. Default output is `.dev/research/`; can route to private notes, knowledge capture, or none.

## Files & Storage

GAL's durable memory is repo-owned and reviewable:

```text
.dev/
├── project.md                 compressed project summary (cold-start first read; the only .dev file committed in this repo)
├── state.md                   active plans index + session continuity
└── plans/<slug>.prompt.md     AI execution work file (mutable task memory)
.dev/plans/<slug>.md           human-readable source plan (transient)
.dev/plans/<slug>.en.md        EN semantic draft for non-English planLanguage (tracked; see manual)
.dev/research/                 research work files (non-durable; findings promote into docs/)
~/.gal/                        machine-local runtime (config, plugins, generated) — see developer guide
```

Private notes are a separate, user-owned boundary (optional Personal Enhancement, default-off) configured in `~/.gal/config/config.json`; GAL Core works fully without it and never assumes a specific notes layout. See [collaborative-tools/local-notes.md](docs/collaborative-tools/local-notes.md).

## Collaborative Tools

Optional external tools that enhance specific lanes. **GAL never auto-installs them and works fully without any of them**; each goes through a shared preflight (`applicable → available → ready → route / degrade`).

| Tool | What it does | Doc |
| --- | --- | --- |
| structural-retrieval aids | Shared capability lane for GAL's structural retrieval helpers: coarse structure first, then optional symbol-aware refinement | [→](docs/collaborative-tools/structural-retrieval.md) |
| graphify | Converts repo files into a knowledge graph (`graphify-out/`); GAL consumes it during planning and review to surface cross-module coupling | [→](docs/collaborative-tools/graphify.md) |
| codebase-memory-mcp | Live MCP-based structural and symbol lookup that can refine targeting after native file detection | [→](docs/collaborative-tools/codebase-memory-mcp.md) |
| Playwright MCP | Managed browser capability for test runs, design audits, dynamic-page research, and browser-visible MCP evaluation; isolated and headless by default | [→](docs/collaborative-tools/playwright-mcp.md) |
| OpenCLI | Turns websites, browser sessions, Electron apps, and local tools into reusable CLI commands; reuses logged-in sessions | [→](docs/collaborative-tools/opencli.md) |
| gstack | Garry Tan's (YC) startup-experience AI agents; adds optional planning-review and engineering-review lanes to the GAL planning flow | [→](docs/collaborative-tools/gstack.md) |
| local-notes | Optional external-notes routing (Obsidian/Logseq/etc.) for the local-first research lane | [→](docs/collaborative-tools/local-notes.md) |

Remote (cross-machine) task execution is a core GAL capability — see [docs/remote-execution.md](docs/remote-execution.md) — not a collaborative tool.

Preflight contract: [checking-contract](docs/collaborative-tools/checking-contract.md).

## Learn More

| Doc | What it owns |
| --- | --- |
| [getting started](docs/getting-started.md) | install → init → your first plan, in one linear pass |
| [user manual](docs/manual.md) | runtime selection, config, model routing, MCP, working hours — everything for running gal |
| [developer guide](docs/devguide.md) | maintainer: codebase + `~/.gal/` structure, making changes |
| [contributing](docs/contributing.md) | how to contribute to gal itself |
| [plugins/gal-core/commands/commands.md](plugins/gal-core/commands/commands.md) · [plugins/gal-core/agents/agents.md](plugins/gal-core/agents/agents.md) · [plugins/gal-core/workflows/coding.md](plugins/gal-core/workflows/coding.md) | canonical control-plane, agent, and workflow contracts |
| [structural-retrieval capability](docs/collaborative-tools/structural-retrieval.md) | routing surface for graphify + codebase-memory-mcp as one structural-retrieval aid group |
| [docs/collaborative-tools/](docs/collaborative-tools/) | one contract per optional tool |

## References

- [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done)
- [GitHub Spec Kit](https://github.com/github/spec-kit)
- [gstack](https://github.com/garrytan/gstack)
- [rtk](https://github.com/rtk-ai/rtk): filters and compresses command outputs before they reach LLM context. Strongly recommended.

## License

MIT — See [LICENSE](LICENSE).
