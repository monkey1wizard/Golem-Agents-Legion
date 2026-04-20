# Golem Agents Legion (GAL)

English | [繁體中文](README.zh-Hant.md)

GAL is an AI working system designed to make development more structured while allowing you to switch between AI tools without losing context. Its core is a `/gal` control plane plus 11 clearly separated Golem Agents, forming a document-driven development model. All durable state is stored in local Markdown files such as `.dev/` and `docs/plans/`, allowing GitHub Copilot, Gemini CLI, and Codex CLI to share the same workflow and work files.

Its state management draws from the phase-based discipline in [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done): explicit state in `.dev/state.md`, verification gates, and a structured execution lifecycle. That is what allows `/gal status` and `/gal whats-next` to project the current state of work in a repo.

- [gstack](https://github.com/garrytan/gstack) has a strong influence on GAL's specialist workflow semantics, but in GAL it is an optional collaborative tool, not a core dependency. See [docs/collaborative-tools/gstack.md](docs/collaborative-tools/gstack.md).

## Quick Start

1. Clone this repository.
`git clone https://github.com/monkey1wizard/golem-agents-legion.git`

2. Run setup from the repo root. Use `./scripts/Setup-Machine.ps1` on Windows and `./scripts/setup-machine.sh` on macOS/Linux.

3. Inside a target repo, open GitHub Copilot, Gemini CLI, or Codex CLI, then run:

```text
# Copilot / Gemini CLI (slash-command surface)
/gal init

# Codex CLI (skill mention surface, uses $ instead of /)
$gal init
```

## Control Commands

| Command | Purpose |
| --- | --- |
| `/gal init` | Initialize a repo by creating `.dev/project.md` and `.dev/state.md` |
| `/gal status` | Full state projection: active plans, review and test status, blockers, continuity |
| `/gal whats-next` | Recommend one next action |
| `/gal wrap-up` | Converge work by writing `### Handoff Notes` and `## Session Continuity` |
| `/gal research` | Enter the research workflow |
| `/gal deep-research` | Enter the multi-source research workflow with cross-review |
| `/gal pipeline` | Run tasks automatically through implementer → tester → reviewer → verifier |

## Development Workflow

```text
init -> planning ─┬─ (scoped feature) ──────────────> plan-to-prompt -> gal-pipeline
                  ↓                                        ↑
                  │                                        │
                  └─ (structural change) -> deep-planning ─┘
```

Not every change needs the full path. A small scoped feature can go from `/planning` directly to `/plan-to-prompt`. Only structural changes need `/deep-planning` first.

### planning

`/planning` turns a new request into a formal source plan and writes it to `docs/plans/<plan-slug>.md`.

The point of this stage is to turn goals, requirements, and assumptions into a stable human-readable document, while recording unresolved items in `## Open Questions`. `/planning` does not create the execution work file. It only decides whether the plan is ready to move directly into `/plan-to-prompt` or should go through `/deep-planning` for more convergence and architectural review.

### deep-planning

`/deep-planning` takes an existing source plan and pushes it toward implementation readiness. It writes planning material back into the same `docs/plans/<plan-slug>.md` file and records architecture review results in `## Review Results > ### Architecture Review` and `## Approval > Architect review`. If the architecture review still has blocking issues, the work remains in deep-planning until those are resolved. Only once the plan is sufficiently converged does it move into `/plan-to-prompt`.

If the change also needs business, design, or engineering review, those should be treated as advanced planning-stage review passes that strengthen the plan, not replace it.

### plan-to-prompt

`/plan-to-prompt` converts the source plan into `.dev/plans/<plan-slug>.prompt.md` for the execution stage and updates state. After that you can run `/gal status` or `/gal whats-next`, which inspect the execution work file to answer what is happening now. `/gal pipeline` only works once that execution work file exists. If the source plan changes later, rerun `/plan-to-prompt` so the plan and prompt stay aligned.

## Golem Agents

GAL is built around 11 specialized agents, each with its own `.agent.md` definition. The design principles are:

- **lean prompts**: each agent loads only its own role definition instead of wasting context on every possible concern
- **independence**: separate tester, reviewer, and verifier roles improve trust in the result
- **composability**: you enable the agents needed for the risk level of the task instead of always loading everything

Besides using `/gal` commands, you can also invoke a `golem-` agent directly for a specific kind of work.

### Classification

| Class | How it activates | Members |
| --- | --- | --- |
| **Utility** | callable directly at any time | debugger, scribe |
| **Domain** | consulted by commands or directly by the user | architect, analyst, designer, researcher, librarian |
| **Pipeline** | chained automatically by `/gal pipeline` | implementer, tester, reviewer, verifier |

### Utility Agents

| Agent | Responsibility |
| --- | --- |
| **debugger** | scientific bug investigation: hypothesis, validation, and root-cause confirmation before any fix |
| **scribe** | daily work log plus curfew enforcement |

### Domain Agents

Domain agents provide specialist advice and can be invoked at any stage.

| Agent | Responsibility |
| --- | --- |
| **architect** | adversarial plan review: trade-offs, over-design detection, bug surface, and public API risk |
| **analyst** | business logic review: ROI, domain correctness, and user impact |
| **designer** | visual design: UI/UX and design-system consistency |
| **researcher** | local-first research and structured synthesis with source attribution |
| **librarian** | writes into the Obsidian vault: inbox processing and knowledge extraction |

### Pipeline Agents

Pipeline is GAL's automated execution core. When you run `/gal pipeline`, each task passes through four agents in order:

```text
T-NNN ──> implementer ──> tester ──> reviewer ──> git commit ──> T-NNN+1
               ↑                         │
               │                         ↓
     auto-fix by review result <────── REJECT

all tasks complete: ──> verifier ──> confirm the plan goal was actually achieved
```

| Agent | Responsibility | Key rule |
| --- | --- | --- |
| **implementer** | implement the current `T-NNN` task against the plan | if it hits an architectural boundary or the plan is insufficient, it must stop and return to `/deep-planning` |
| **tester** | write or extend tests from the spec and public API, then verify the implementation | it must not read implementation code and must use a different model than implementer |
| **reviewer** | review the diff for risk, correctness, and completeness | if it finds blocking issues, work must go back to implementer. its model should differ from implementer and should not be weaker |
| **verifier** | work backward from the original goal after all tasks are done | it decides whether the plan can close and what knowledge should be absorbed back into `docs/` |

Pipeline authority only comes from `/gal pipeline`. Directly invoking a pipeline agent is consultation, not formal execution. Domain and utility agents can still be called directly at any time.

### AI Model And Agent Rules

In pipeline flow, GAL enforces model separation for review and test work:

- tester **must** use a different model than implementer
- reviewer **should** use a different model than implementer, and should not be weaker
- planning and architect **should ideally** use different models

These rules are configured in `model-roles.local.md`.

### Curfew System

All agents obey curfew boundaries:

- **22:00**: non-scribe agents are blocked if the daily diary has not been written
- **22:00–23:00**: if the diary is already written, agents may suggest `/gal wrap-up`
- **23:00**: all agents stop, including scribe
- **Override**: the user can say `override curfew` for a one-time exception

## Project Files

| Path | Purpose |
| --- | --- |
| `.dev/project.md` | repo summary, stack, goals, and constraints |
| `.dev/state.md` | active plan index, blockers, and session continuity |
| `.dev/learnings.jsonl` | repo-local institutional memory |
| `.dev/plans/<plan-slug>.prompt.md` | AI execution work file |
| `CLAUDE.md` | repo-local operational notes |
| `DESIGN.md` | repo-level design governance |
| `docs/designs/<plan-slug>/` | plan-bound design assets |
| `docs/plans/<plan-slug>.md` | human-readable plan document |
| `docs/qa-reports/` | QA reports |
| `docs/research/` | research reports |

### Execution Work File

After you run `/plan-to-prompt`, GAL creates `.dev/plans/<plan-slug>.prompt.md`. You can think of it as the working board for that task. In normal use, the two most important things to read first are:

- `## Status > Workflow`: which phase the work is in right now
- the write-back sections: where planning, testing, review, and handoff results are recorded

Normally you do not need to edit `Workflow:` by hand. It is initialized as `DRAFT` when the execution work file is created and then moves forward automatically as work progresses.

#### Workflow States

| State | Meaning |
| --- | --- |
| `DRAFT` | the execution work file has just been created and execution has not started |
| `IMPLEMENT` | implementation of a `T-NNN` task is in progress |
| `TEST` | implementation is done and testing is in progress |
| `REVIEW` | testing passed and code review is in progress |
| `REVIEW — N blocking issues found` | review found blocking issues and the work must be fixed before review runs again |
| `ABSORBED` | the goal has been confirmed complete and the plan is ready to close |

#### Write-Back Sections

| Section | When you look at it | Purpose |
| --- | --- | --- |
| `## Open Questions` | when requirements, assumptions, or boundaries are still unresolved | central list of unresolved issues |
| `## Tasks` | when you need to know what work the plan actually contains | task list and basis for pipeline execution |
| `## Analyze` | when you want to check whether the current diff still matches the original plan scope | records `/review` drift analysis |
| `## Review Results` | when you want review outcomes | collects review results from different specialists |
| `## Test Plan` | before testing starts, when you want to know what should be verified | records intended verification scope |
| `## Test Results` | after tests or QA are complete | records test outcomes |
| `### Handoff Notes` | when you pause and need to know where work last stopped | provides resume context for the next session |
| `## Ship` | when preparing to merge or after shipping | records ship-stage results |
| `## Deploy` | once deployment has started | records deployment result and verification |

`/gal status` and `/gal whats-next` read these written sections to determine state and next action. They do not rely on one field alone.

## Research Workflow

Research is a workflow separate from development and can run in parallel with development work.

| Mode | Best for | Flow | Source requirement |
| --- | --- | --- | --- |
| `/gal research` | standard structured investigation | RESEARCH -> VERIFY -> DOCUMENT | enough to answer the question |
| `/gal deep-research` | high-risk, high-ambiguity, or cross-topic investigation | RESEARCH -> SYNTHESIZE -> CROSS-REVIEW -> VERIFY -> DOCUMENT | must attempt at least 5 sources |

Both modes require VERIFY to be performed by a model different from the research author. CROSS-REVIEW in `deep-research` is a consistency review across sources, not architecture or business review. The type of gap determines where the workflow falls back:

```text
IDLE -> RESEARCH -> SYNTHESIZE -> CROSS-REVIEW -> VERIFY -> DOCUMENT -> DONE
         ↑            ↑             ↑             |
         └ evidence ──┴ synthesis ──┴ source/ref ─┘
```

## Collaborative Tools

GAL can also use collaborative tools. These tools strengthen specific query capabilities or agent skills across the GAL workflow, but none of them are required by `/gal`, so GAL still works fully even if none of the tools below are installed. You can install them by running `Setup-Tools`.

### Shared Preflight Model

Every collaborative tool goes through the same 5-state preflight check before use:

```text
applicability → availability → initialization status → readiness → route / degrade
```

| State | Meaning |
| --- | --- |
| `not-applicable` | The current lane or task does not need this tool — skip silently |
| `unavailable` | The machine or runtime cannot access the tool — use the fallback path |
| `available-but-needs-init` | The tool exists but first-time setup is incomplete — do not auto-initialize during normal flow |
| `available-but-not-ready` | Installed and initialized, but the current repo or task lacks the required artifacts |
| `ready` | Applicable and all preconditions satisfied — route into the tool |

Core rules: never auto-install, never auto-initialize, never hide missing capabilities behind vague success language. Every tool-enabled lane has an explicit degrade path. Full spec in [docs/collaborative-tools/checking-contract.md](docs/collaborative-tools/checking-contract.md).

### graphify

A graph-structured analysis tool. It analyzes all files in a folder, writes its outputs into `graphify-out/`, and can improve later AI query capability. See [docs/collaborative-tools/graphify.md](docs/collaborative-tools/graphify.md).

### OpenCLI

Turn websites, browser sessions, Electron apps, and local tools into deterministic interfaces for humans and AI agents. Reuse your logged-in browser, automate live workflows, and crystallize repeated actions into reusable CLI commands. See [docs/collaborative-tools/opencli.md](docs/collaborative-tools/opencli.md).

### gstack

Created by Garry Tan, President & CEO of Y Combinator. It turns his startup experience into AI agents. See [docs/collaborative-tools/gstack.md](docs/collaborative-tools/gstack.md).

### Remote Worker

Cross-machine remote task dispatch and result retrieval. See [docs/collaborative-tools/remote-worker.md](docs/collaborative-tools/remote-worker.md).

### Godot C Sharp

Existing commands operate on Godot 4 C# repos through conventions, skills, and MCP tools. See [docs/collaborative-tools/godot.md](docs/collaborative-tools/godot.md).

### AI-First Game Assets

Uses ComfyUI as the generation entry point and downstream tools for cleanup and export. See [docs/collaborative-tools/graphworkflow.md](docs/collaborative-tools/graphworkflow.md).

## Personalization

Everything related to machine-local configuration that does not belong on the README front page is collected in [docs/personalization.md](docs/personalization.md). It covers how to fill environment placeholders, how to choose and reconfigure execution environments, model routing, MCP overrides, which `.local.*` files should hold local secrets and paths, and when setup needs to be rerun. If you need to change the AI tools you use on this machine, model-role mappings, or MCP settings, start there.

## Documentation

`docs/` is mainly for fast human reading and lookup. `docs/collaborative-tools/` is the quick-start and index layer for collaborative tools and workflow guides.

| Path | Purpose |
| --- | --- |
| [docs/command-index.md](docs/command-index.md) | command map |
| [docs/devguide.md](docs/devguide.md) | maintainer guide |
| [docs/personalization.md](docs/personalization.md) | local model routing, MCP overrides, and other personalization guidance |
| [docs/collaborative-tools/checking-contract.md](docs/collaborative-tools/checking-contract.md) | shared preflight checking contract for collaborative tools |
| [docs/collaborative-tools/graphify.md](docs/collaborative-tools/graphify.md) | graph-structured analysis tool |
| [docs/collaborative-tools/opencli.md](docs/collaborative-tools/opencli.md) | OpenCLI guide and when to use it |
| [docs/collaborative-tools/gstack.md](docs/collaborative-tools/gstack.md) | gstack collaborative tool contract for planning and specialist agent integration |
| [docs/collaborative-tools/remote-worker.md](docs/collaborative-tools/remote-worker.md) | remote worker topology, ownership, and patch-first convergence |
| [docs/collaborative-tools/godot.md](docs/collaborative-tools/godot.md) | Godot C# workflow guide |
| [docs/collaborative-tools/graphworkflow.md](docs/collaborative-tools/graphworkflow.md) | AI-first game asset workflow guide |

## References

- [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done)
- [GitHub Spec Kit](https://github.com/github/spec-kit)
- [gstack](https://github.com/garrytan/gstack)

## License

MIT — see [LICENSE](LICENSE).
