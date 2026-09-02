# Golem-Agents-Legion

**English** · [日本語](docs/i18n/ja/README.ja.md) · [繁體中文](docs/i18n/zh-Hant/README.zh-Hant.md)

A cross-provider AI working system built in Rust. GAL operates natively across Claude Code, Codex CLI, GitHub Copilot, Antigravity CLI, and opencode, allowing you to freely mix models from different providers within a single development lifecycle. Its compiled binary architecture minimizes token overhead when transferring context across providers, while repo-owned Markdown files keep planning, implementation, testing, review, and research seamlessly portable between tools.
You command a legion of golems like a master spellcaster, with each golem agent bound to a distinct role in your workflow. Hence the name Golem-Agents-Legion.

## What is GAL

- **Repo-owned state** (`.dev/`): Replaces vendor-locked chat memory, enabling context persistence across AI tools.
- **One unified contract**: A control-plane operations surface (`/gal …`) and specialized agents enforce consistent workflows across five runtimes.

## Why GAL

- **Cross-provider by design**: Planning, implementation, testing, and audit phases dispatch to distinct provider agents via `/gal pipeline`. The REFINE-LOCK loop converges plans; cross-model verification is structural, not optional.
- **Rust-native orchestration**: The `gal` binary manages orchestration, state projection, and adapter generation as a compiled binary. Provider context transfers carry lower structural overhead than prompt-only approaches, independent of total token consumption per pipeline run.
- **Cost control**: Execution models per phase remain configurable via `config.json#executorRouting`.
- **No provider lock-in**: Workflows stay standardized and independent of any specific AI provider. Switch providers at any time without re-architecting your project state.
- **State continuity**: Execution states record directly into `.dev/`. Interrupted pipeline runs resume from the exact recorded breakpoint.

## Get Started

The primary GAL methodology utilizes **install mode**. Cloning this repository is unnecessary.

**Prerequisites:** Requires prior setup of one supported AI coding runtime, including Claude Code, Codex CLI, GitHub Copilot, Antigravity CLI, or opencode.

1. **Install the `gal` CLI:**

   ```bash
   curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | bash
   ```

   ```sh
   irm https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.ps1 | iex
   ```

   Alternative installation methods including Homebrew, winget, and cargo are detailed within the [user manual](docs/manual.md#installing-gal).

2. **Project initialization**: Open a supported runtime within the target project and execute `/gal init`. Codex utilizes `$gal init`. This action scaffolds the `.dev/` directory and generates per-agent adapter files.

3. **Workflow execution**: Execute `/gal status` to confirm repository initialization and review subsequent steps. Execute `/planning` to convert a request into an initial plan.

These steps represent the complete path to a functional, initialized repository. Optional integrations are not required for this baseline state. Subsequent content provides on-demand reference material.

### Feature lifecycle

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
/gal finalize
```

The `/gal init` command scaffolds the repository once following the prior steps. Subsequent nodes possess dedicated sections within [How GAL Works](#how-gal-works). The planning phase resides in [Planning Phase](#planning-phase), execution in [Pipeline](#pipeline), and landing in [Finalize](#finalize).

## Files & Storage

GAL files reside in two distinct locations. Durable, repo-owned state exists **within the project folder**, and machine-local runtime files exist **within the home folder (`~`)**.

**Project folder contents** — durable, reviewable, and diffable workflow state created by `gal init`:

```text
.dev/
├── project.md                 compressed project summary (cold-start first read)
├── state.md                   active plans index + session continuity
├── plans/<slug>.md            human-readable source plan (transient)
├── plans/<slug>.prompt.md     AI execution work file (mutable task memory)
├── plans/<slug>.en.md         EN semantic draft for non-English planLanguage (tracked, see manual)
└── research/                  research work files (non-durable, findings are promoted to docs/)
CLAUDE.md · AGENTS.md · GEMINI.md · .github/copilot-instructions.md · .agents/rules/gal.md
                               generated per-agent adapters (regenerate with `gal init --force`)
```

**Home directory contents** — machine-local runtime components, strictly excluded as sources of truth:

```text
~/.gal/
├── config/config.json         user-owned machine config: personalization, secrets, executorRouting — preserved across reinstalls
├── plugins/gal/               GAL-managed canonical plugin root (the runtime content owner)
├── generated/                 GAL-produced projections, rebuildable
├── state/plugins.lock.json    projection registry lockfile
└── active/<runtime>/          stable shortcut targets for AI tools
```

Provider-visible surfaces including `~/.claude/skills/gal` and `~/.copilot/skills/` function as canonical root projections rather than secondary truth sources. Full layout documentation resides in [docs/architecture.md → Runtime Topology](docs/architecture.md#repository--runtime-topology).

Private notes operate as a separate, user-owned boundary. This represents an optional Personal Enhancement, disabled by default, and configured within `~/.gal/config/config.json`. GAL Core maintains full functionality without this feature and strictly avoids assuming specific notes layouts. This feature constitutes an optional capability rather than a collaborative tool. Refer to [optional-capabilities.md](plugins/gal-core/conventions/optional-capabilities.md) for binding contracts and [docs/manual.md](docs/manual.md) for setup instructions.

## How GAL Works

### Commands

| Family | Commands |
| --- | --- |
| Control plane | `/gal` (bare initiates auto-detect next step) plus subcommands `init · status · whats-next · wrap-up · pipeline · finalize · research · deep-research` |
| Agent routes | `/gal <golem-name>` (directly callable golems) and `/gal discuss <role>` (in-context consult) |
| Planning | `/planning` · `/deep-planning` · `/refining-plan` · `/plan-to-prompt` |
| Standalone skills | `/adversarial-review` (adversarial review covering plans, diffs, docs, or decisions), `git-commit-msg` (Conventional Commit message generation from staged changes), and `/text-flowcharts` (render branching logic as monospaced text decision-tree diagrams) |

The Codex runtime invokes identical names as skills utilizing the `$` prefix (e.g., `$gal status` instead of `/gal status`). Complete contract details reside in [plugins/gal-core/commands/commands.md](plugins/gal-core/commands/commands.md).

### Golem agents

Ten specialized agents split into two classes: directly callable and orchestrated-only. Directly callable agents are `architect`, `analyst`, `designer`, `releaser`, `debugger`, `steward` , you can call them via `/gal <role>` during planning phase and disussion.

Full capability table, invocability matrix, and dual-mode details: [docs/manual.md → Golem Agents](docs/manual.md#golem-agents). Authoritative roster: [agents.md](plugins/gal-core/agents/agents.md). Workflow contracts: [coding.md](plugins/gal-core/workflows/coding.md).

### Planning Phase

- `/planning`: creates source plans located at `.dev/plans/<type>-<slug>.md` based on requests.
- `/deep-planning`: executes an expand-then-converge architectural review. This involves a mandatory architect review, conditional analyst and designer reviews, and a steward plan-structure check. This step is mandatory for Protected Paths or structural changes and outputs `ARCH_REVIEW: CLEAR`. **Strict requirement: User resolution of all open questions is mandatory prior to initiating `/refining-plan`.**
- `/refining-plan`: writes the implementation contract into the source plan. The `## Tasks` (T-NN) and `## Test Plan` (TP-NN) sections iterate through an architect and tester Definition-of-Ready gate until convergence, outputting `ENG_REVIEW: CLEAR`.
- The adversarial review process governs all review gates. This enforces steel-man argumentation, refute-by-default logic, and explicit APPROVE, REVISE, or REJECT verdicts where REVISE creates a hard stop-line. The `/adversarial-review` command provides standalone access to this methodology.
- Human approval recorded in the `## Approval` section serves as the final manual review to confirm the draft. This explicit approval is a mandatory prerequisite before the `/plan-to-prompt` command can generate the `.dev/plans/<slug>.prompt.md` execution work file.

Complete planning semantics reside in [plugins/gal-core/workflows/coding.md](plugins/gal-core/workflows/coding.md#stage-35--definition-of-ready-gate-refine-lock-loop).

### Pipeline

```text
per task (T-NN):

  ORCHESTRATOR
       |
       ├── Test-first: required ──► CODER scaffold (conditional, no commit)
       |                                  |
       |                                  ▼
       |                         TESTER expected red (no commit)
       |                                  |
       |                                  ▼
       |                         CODER implement-green (no commit)
       |                                  |  test paths frozen
       |                                  ▼
       |                   ORCHESTRATOR same-command red-to-green
       |                                  |
       |                                  ▼
       |                         AUDITOR dirty-tree review
       |                                  |
       |                                  ▼
       |                    ORCHESTRATOR implementation commit
       |                                  |
       |                                  ▼
       |                         state convergence + state commit
       |
       └── Test-first: not-applicable ──► legacy implement → test → audit

  all tasks done → ORCHESTRATOR goal-backward verify → /gal finalize
```

When the execution prompt carries `Pipeline Contract: test-first-v1`, scaffold is conditional on the task contract. TESTER proves the locked probes red first; CODER then implements against those probes while leaving the test paths frozen. The implementation remains uncommitted through green verification and the dirty-tree audit. Only after those gates pass does ORCHESTRATOR create the delayed implementation commit, followed by the separate state-recording commit. Markerless or not-applicable tasks retain the legacy `implement → test → audit` path.

The canonical ownership chain is:

```text
plugins/gal-core/ (source contracts)
          ↓ render
~/.gal/plugins/gal/ (canonical runtime root)
          ↓ project
copilot · antigravity · codex · opencode · claude
```

The runtime names above are the entries in `gal_foundation::runtime::VALID_RUNTIMES` at this revision. Provider-visible surfaces are projections; they are not alternate owners of pipeline semantics.

Codex environment stops are separate from workflow results. A receipt or executor-log write denial, missing executor, or unavailable adapter is a hard environment stop for that Codex phase; rerun the exact command after the environment is repaired. Do not infer scaffold, red, green, audit, or commit success from a stopped environment.

During execution, each pipeline phase dispatches to a routed executor defined in `config.json#executorRouting`, allowing the implementer, tester, and auditor roles to utilize **distinct vendor coding agents**. This ensures independent cross-model verification by construction.

After each task convergence gate, the pipeline runs the handback checker to decide whether it may continue, stop at a retry ceiling, pause for a human-required blocker, or hand off to finalize. Pipeline execution still escalates to the user if a single task fails verification more than three times, preventing infinite loops. The orchestrator controls the correctness gate, and the `golem-auditor` manages deep-performance and security audits per task.

Users can execute remote cross-machine tasks via **SSH**. For instructions, refer to [docs/manual.md](docs/manual.md#remote-execution-ssh-dispatch-lane) for specifications. Complete workflow semantics reside in [plugins/gal-core/workflows/coding.md](plugins/gal-core/workflows/coding.md).

### Finalize

- The `/gal finalize` command acts as a thin orchestrator to sequence a destructive completion landing. To enforce zero-trust re-verification, execution is strictly gated on a passing full-mode `gal finalize-check` receipt and successful backward-tracing goal verification by the orchestrator. The sequence encompasses:
  - **Holistic review**: executes a whole-branch evaluation across all combined tasks.
  - **Document verification (Doc-sync)**: the `STEWARD` extracts durable knowledge into `docs/`, establishing a strict commit-hash gate.
  - **Merge, delete, and close**: executes a `git merge` into main (if need), tears down any worktree, and enables the `ORCHESTRATOR` to perform plan-file deletion to close the lifecycle. Releases occur only when shipping an actual artifact.

The `/gal wrap-up` command operates differently. It functions as a non-destructive session **pause** callable at any time to record mid-flight continuity, avoiding any landing or closing actions.

### Research

This workflow operates independently of the Coding Flow, executing concurrently with development or as a standalone investigation.

- The `/gal research` command executes a RESEARCH → VERIFY → DOCUMENT sequence.
- The `/gal deep-research` command handles higher-ambiguity topics by inserting SYNTHESIZE and CROSS-REVIEW operations prior to VERIFY. It requires attempting a minimum of five sources.

The VERIFY phase requires an independent model—distinct from the findings author—to reverse-check all cited references. Outputs default to `.dev/research/` to prioritize repo-owned evidence. Routing findings to personal external notes requires an opted-in machine-local notes backend. Complete contract details reside in [plugins/gal-core/workflows/research.md](plugins/gal-core/workflows/research.md).

## Integrations

Optional external tools enhance specific operational lanes. **GAL avoids auto-installation and maintains full functionality without these tools**. Integrations process through a shared five-state preflight sequence covering applicability, availability, initialization status, readiness, routing, and degradation.

| Tool | Functionality | Documentation |
| --- | --- | --- |
| codebase-memory-mcp | Executes live MCP-based structural and symbol lookups to refine targeting following native file detection | [→](docs/integrations.md#codebase-memory-mcp) |
| graphify | Transforms repository files into a knowledge graph outputting to `graphify-out/`. GAL consumes this data during planning and review to identify cross-module coupling | [→](docs/integrations.md#graphify) |
| OpenCLI | Converts websites, browser sessions, Electron apps, and local tools into reusable CLI commands, leveraging active logged-in sessions | [→](docs/integrations.md#opencli) |
| Playwright MCP | Provides user-wired browser capabilities supporting test runs, design audits, dynamic-page research, and browser-visible MCP evaluations | [→](docs/integrations.md#playwright-mcp) |

Comprehensive integration details, setup instructions, and routing contracts reside in [docs/integrations.md](docs/integrations.md).

## Documentation

This section is the **single authority on document responsibility boundaries**. Every top-level document has one fixed job. Content not matching a document's row belongs in the document whose row it does match. Move it rather than duplicating it. No other document redefines these boundaries. They link here instead.

| Document | Job | The question it answers |
| --- | --- | --- |
| [README](README.md) (this file) | Public entry: what GAL is, why it exists, quick start, workflow overview | "What is this and how do I start using it?" |
| [user manual](docs/manual.md) | End-user operations: install, first run, workflows, config, personalization, executors, golems | "How do I operate GAL day to day?" |
| [integrations](docs/integrations.md) | One section per optional integration | "How does GAL work with tool X?" |
| [contributing](CONTRIBUTING.md) | Contributor entry: gates, workflow skeleton, reading order | "I want to contribute — what is the first step?" |
| [architecture](docs/architecture.md) | **What the system is and why, shown as diagrams.** Structure, ownership maps, and decision records. Diagrams carry the explanation and prose supports them. All user can read this to understand how GAL fits together, not only a developer | "How does this fit together, and why is it shaped this way?" |
| [developer guide](docs/devguide.md) | **How to change it**: procedures, inner loop, operations. Prose and step lists, never a second copy of structure. **Developer-only** — assumes a source checkout and intent to modify GAL | "I want to change X — what are the steps?" |
| [naming](docs/naming.md) | Semantic authority for every core term, plus the retired-terms gate input | "What does this word mean here?" |
| [plugins/gal-core/commands/commands.md](plugins/gal-core/commands/commands.md) · [plugins/gal-core/agents/agents.md](plugins/gal-core/agents/agents.md) · [plugins/gal-core/workflows/coding.md](plugins/gal-core/workflows/coding.md) | Defines canonical control-plane operations, agent responsibilities, and workflow contracts | "What is the contract?" |

**Reading order for a new contributor:** `CONTRIBUTING.md` (start) → `docs/architecture.md` (understand) → `docs/devguide.md` (change).

**Audience split between the two developer-facing docs.** `architecture.md` is diagram-first and open to any reader. A user who never intends to touch the code can read it to understand what GAL does to their machine and why. `devguide.md` is procedure-first and assumes the reader has a source checkout and is about to modify GAL. When adding content, ask which of the two the reader is. A field-level schema table, a test-file location, or a rebuild command belongs in `devguide.md` even when it describes structure.

The canonical contracts under `plugins/gal-core/` always override any doc summary of them. Documentation formatting, naming, and translation policy is a separate concern owned by [devguide → Documentation Conventions](docs/devguide.md#documentation-conventions).

## References

- [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done)
- [GitHub Spec Kit](https://github.com/github/spec-kit)
- [gstack](https://github.com/garrytan/gstack)
- [rtk](https://github.com/rtk-ai/rtk): A CLI proxy engineered to filter and compress dev-command output including git, cargo, and test runners prior to reaching the model context. This tool exists independently of GAL integrations and operates without GAL detection or dependency. Utilization alongside GAL is strongly recommended. Pipeline executions trigger numerous shell commands per task, and output trimming effectively maximizes runtime context budgets.

## License

Licensed under the MIT License. Refer to [LICENSE](LICENSE).
