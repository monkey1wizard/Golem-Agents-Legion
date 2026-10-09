# Golem-Agents-Legion

**English** · [日本語](docs/i18n/ja/README.ja.md) · [繁體中文](docs/i18n/zh-Hant/README.zh-Hant.md)

GAL is a document-driven AI developer workflow engine built in Rust. It provides a structured engineering lifecycle across planning, implementation, testing, review, and research using repository-owned Markdown files (`.dev/`). GAL works seamlessly within a single AI coding tool—such as Claude Code, Codex CLI, GitHub Copilot, Antigravity CLI, or opencode—while giving you the freedom to switch or combine models across providers without vendor lock-in. The compiled binary architecture drives development under deterministic quality gates.

Like commanding your own legion of golems, each specialized golem agent focuses on a distinct role in your workflow to accomplish engineering tasks collaboratively. This is the origin of the project name Golem-Agents-Legion.

## Core Capabilities

- **Single-tool ready, cross-runtime flexible**: Operates fully out-of-the-box inside your primary AI runtime (Claude Code, Codex CLI, GitHub Copilot, Antigravity CLI, or opencode) without requiring multiple subscriptions. easily switch tools or route specific phases to different models whenever desired.
- **Document-driven engineering lifecycle**: Uses phased review gates and the REFINE-LOCK loop to converge requirements deterministically before code changes begin.
- **Repository-owned state and breakpoint resume (`.dev/`)**: Preserves the complete development context in local Markdown files rather than vendor-locked chat history, interrupted pipeline runs resume precisely from their last recorded checkpoint.
- **Unified control plane and zero vendor lock-in**: Enforces consistent workflows across all supported runtimes via a shared operations surface (`/gal …`) and specialized golem contracts, keeping your project independent of any single platform.
- **Compiled Rust engine**: Fast native binary handles orchestration, state projection, and adapter generation locally, eliminating the token and structural overhead of prompt-only frameworks.
- **Granular model routing and cost control**: Configure the most suitable models for each phase (such as planning, implementation, or audit) in `~/.gal/config/config.json#executorRouting`.

## Quick Start

You can install and use GAL directly without cloning this repository.

**Prerequisite:** Ensure at least one supported AI runtime is installed: Claude Code, Codex CLI, GitHub Copilot, Antigravity CLI, or opencode.

1. **Install the `gal` CLI:**

   ```bash
   curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | bash
   ```

   ```sh
   irm https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.ps1 | iex
   ```

   For other installation options (including Homebrew, WinGet, and building from source), see [docs/setup.md](docs/setup.md).

2. **Initialize your project.** Open your AI runtime in the target project directory and run `/gal init` (or `$gal init` in Codex CLI). This initializes the `.dev/` directory and generates the necessary agent adapters.

   **Check instruction loading.** Confirm that your coding agent loads GAL's `AGENTS.md`, the only repository instruction root GAL provides. If your repository contains `CLAUDE.md` or a similar file, your agent's loading design may cause it to skip `AGENTS.md`. Consult your coding agent's documentation for supported filenames and loading precedence. See [setup guidance](docs/setup.md#initializing-a-repository-gal-init).

3. **Start working.** Run `/gal status` to check repository status and view suggested next steps. Run `/planning` to generate an initial plan from your task requirements.

The repository is now initialized and ready to use. Optional executor routing and live provider checks require additional configuration. Refer to the [Codex pipeline setup](docs/setup.md#codex-pipeline-execution-setup) for supported permission modes and the [execution lifecycle](docs/workflows.md#pipeline-execution-order-and-evidence) for dispatch boundaries and evidence stages. For additional topics, see the [documentation map](#documentation-map) below.

### The Feature Lifecycle

```text
/gal init
   ↓
┌── Planning Phase ──────┐
│ /planning              │
│    ↓                   │
│ /deep-planning         │
│    ↓                   │
│ [OQ-completion gate]   │
│    ↓                   │
│ /refining-plan         │
│    ↓                   │
│ [human approval gate]  │
│    ↓                   │
│ /plan-to-prompt        │
└────────────────────────┘
   ↓
/gal pipeline
   ↓
[automatic finalize]
```

After the pipeline verifies the plan goal, it runs finalize as its last step. One normal `/gal pipeline` run therefore ends with a landed plan or with a specific stop, such as an outstanding Owner Acceptance row or a failed gate. A stop does not land the plan and does not accept any outstanding check. Run `/gal finalize` yourself only to enter finalize explicitly or to recover after a stop.

The `/gal init` command creates the repository skeleton once. For detailed operational instructions covering planning phases, pipeline execution, and finalization, see [docs/workflows.md](docs/workflows.md).

## Documentation Map

This table provides an overview of the published documentation. Each topic has one authoritative file, and related documents reference it instead of duplicating rules.

Each description below matches the target document's front matter `description` verbatim, except for the public READMEs, whose descriptions are maintained in their documentation maps. If a discrepancy exists for another document, its front matter takes precedence and this table is updated.

| Document | Single responsibility |
| --- | --- |
| [README.md](README.md) | Public entry point for GAL, providing the project overview, quickstart instructions, feature lifecycle summary, and documentation map. |
| [docs/setup.md](docs/setup.md) | Install the gal binary, register the plugin with supported runtimes, and initialize and maintain repository adapters. |
| [docs/configuration.md](docs/configuration.md) | Manage machine-local settings in `~/.gal/config/config.json`, including executor routing, working hours, plan language, MCP sources, the personal layer, and local-notes routing. |
| [docs/workflows.md](docs/workflows.md) | Explains golem agent roles, invocation methods, planning workflows, automated pipeline execution, finalization, recovery, and authoring helpers. |
| [docs/projection.md](docs/projection.md) | Explains how GAL renders source contracts into the canonical root, projects them across runtime surfaces, and generates repository adapters, projection registries, and MCP manifests. |
| [docs/architecture.md](docs/architecture.md) | Details GAL architectural constraints, crate and layer boundaries, storage topology, migration paths, documentation governance, and the ADR index. |
| [docs/integrations.md](docs/integrations.md) | Reference guide for optional external integrations, covering operational purpose, readiness checks, configuration boundaries, and graceful degradation. |
| [SECURITY.md](SECURITY.md) | Defines supported versions, vulnerability reporting procedures, threat models, and security trust boundaries for GAL. |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution guidelines for GAL, covering branching conventions, commit standards, build and test gates, and review checklists. |

Terminology authorities and Architectural Decision Records (ADRs) are maintained without set translation:

| Document | Single responsibility |
| --- | --- |
| [`docs/glossary.md`](docs/glossary.md) | Single semantic authority for GAL terminology, covering naming rules, the canonical term registry, and retired terms. |
| [ADR](docs/adr/) | Architectural decision records documenting evaluated options and rejected alternatives, indexed in architecture.md. |

## References

- [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done)
- [GitHub Spec Kit](https://github.com/github/spec-kit)
- [gstack](https://github.com/garrytan/gstack)
- [rtk](https://github.com/rtk-ai/rtk): A CLI proxy that filters and compresses terminal output from tools such as `git`, `cargo`, and test runners before sending it to model context. Pairing `rtk` with GAL is strongly recommended. Pipeline tasks execute numerous shell commands, and filtering CLI output preserves context window capacity.

## License

MIT. See [LICENSE](LICENSE).
