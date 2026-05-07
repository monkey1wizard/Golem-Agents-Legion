# Golem Agents Legion (GAL)

English | [繁體中文](README.zh-Hant.md)

GAL is an AI working system that brings structure to development while letting you switch between AI tools without losing context. Its core consists of 12 specialized Golem Agents plus a `/gal` control plane, creating a document-driven development model. Durable state is split across two boundaries: repository-shared state saved as local Markdown files in `.dev/`, `docs/plans/`, and `docs/research/`, plus optional user-personal notes that can be written to a locally configured Obsidian vault. This lets supported runtimes share the same workflow while preserving your personal note space outside the repository.

State management draws from the phase-based discipline in Get Shit Done (GSD): explicit state (`.dev/state.md`), verification gates, and a structured execution lifecycle, letting `/gal status` and `/gal whats-next` project the current state of work for the entire repository.

## Quick Start

1. Clone this repository:
`git clone https://github.com/monkey1wizard/golem-agents-legion.git`

2. Navigate to the project directory and run the installation script. Use `./scripts/Setup-Machine.ps1` on Windows or `./scripts/setup-machine.sh` on macOS/Linux. Once complete, you can start using GAL in your repositories.

3. In your target repository, open your preferred supported runtime, then invoke GAL using that runtime's normal command or skill surface. Common examples:

```text
# Common slash-command surface
/gal init

# Codex CLI (skill mention surface, uses $ instead of /)
$gal init
```

Runtime-specific entry-surface differences live in `scripts/scripts.md` and `docs/devguide.md`.

## Public Commands

| Command | Purpose |
| --- | --- |
| `/gal init` | Initialize repository: create `.dev/project.md` and `.dev/state.md` |
| `/gal status` | Full state projection: active plans, review/test status, blockers, continuity |
| `/gal whats-next` | Recommend the next single action |
| `/gal wrap-up` | Converge work: write `### Handoff Notes` and `## Session Continuity` |
| `/gal research` | Enter the research workflow |
| `/gal deep-research` | Enter multi-source research workflow with cross-review |
| `/gal pipeline` | Automatically chain implementer → tester → reviewer per task. Inserts conditional `golem-security` review if changes touch security-sensitive surfaces, then wraps up with verifier |
| `/planning` | Create a source plan |
| `/deep-planning` | Converge the source plan into an implementable state |
| `/refining-plan` | Write `## Tasks`, `## Test Plan`, and engineering review results into the source plan |
| `/plan-to-prompt` | Generate the execution prompt |

## Development Workflow

```text
         init
          │
          v
       planning
          │
          v
     source plan (docs/plans/*)
          │
     ┌────┴──────────────┐
     │                   │
     │ direct use of     │ if using deep-planning
     │ analyst/designer  │ architect always runs
     │ when content needs│ analyst/designer runs
     │ specialist review │ concurrently if needed
     └────┬──────────────┘
          │ 
          v
     refining-plan
          │ 
          v
     plan-to-prompt
          │
          v
     gal-pipeline
```

### planning

`/planning` converts new requirements into a formal source plan and writes it to `docs/plans/<plan-slug>.md`.

The focus of this stage is organizing your goals and requirements into a stable, human-readable document, recording unresolved items in `## Open Questions`. `/planning` doesn't create the execution work file—it only decides whether the current plan should first go through `/deep-planning` for convergence and architecture review, or move directly to `/refining-plan` to lock in the implementation contract.

### deep-planning

`/deep-planning` performs deep planning on an existing source plan, always activating the architect. Planning materials are organized back into the same `docs/plans/<plan-slug>.md`, and architecture review results are written to `## Review Results > ### Architecture Review` and `## Approval > Architect review` in the plan. If the architecture review surfaces blocking issues, work remains in deep-planning for further refinement. Only when the plan is sufficiently converged for execution does it move down to `/refining-plan`, then `/plan-to-prompt`.

If the plan involves business logic, pricing, permissions, notifications, onboarding, or identity verification, the analyst activates alongside the architect. If it touches customer-facing flows, layout, states, components, or accessibility, the designer also joins. If you only need a specialist review at the business or design level without a full architecture review, you can directly invoke the corresponding domain lane against the plan document.

### refining-plan

`/refining-plan` completes the three key sections that must be finalized before prompt execution: `## Tasks` (listing `T-NNN` tasks), `## Test Plan` (creating `TP-NNN` test matrices corresponding to tasks), and `## Review Results > ### Engineering Review` (marked as CLEAR (`<!-- ENG_REVIEW: CLEAR -->`) or BLOCKING with an explanation). This step bridges the gap between planning and `/plan-to-prompt` through detailed design. `/refining-plan` doesn't involve code implementation, test execution, or `## Status` changes. If gstack is installed, its `plan-eng-review` can serve as an alternative.

### plan-to-prompt

`/plan-to-prompt` converts the plan document into `.dev/plans/<plan-slug>.prompt.md` using a template for the execution phase and updates state. Afterward, you can run `/gal status` or `/gal whats-next`, and the system will scan the execution work file to provide guidance. Once the execution work file exists, `/gal pipeline` can run normally. If the plan document changes after conversion, rerun `/plan-to-prompt` to ensure consistency between the plan document and execution work file.

## Golem Agents

GAL's core consists of 12 specialized agents, each with an independent `.agent.md` definition. The separation of duties design principles are:

- **Lean prompts**: Each agent loads only its own role definition, avoiding context window waste.
- **Independence**: Independent tester, reviewer, and verifier ensure verification results are credible.
- **Composability**: Agents activate based on task risk level—not all at once.

You can use `/gal` commands or invoke `golem-` agents directly for specific work.

### Classification

| Category | Activation | Members |
| --- | --- | --- |
| **Utility** | Called directly at any time | debugger, notewriter |
| **Domain** | Consulted via command or by user | architect, analyst, designer, researcher, security, releaser |
| **Pipeline** | Automatically chained by `/gal pipeline` | implementer, tester, reviewer, verifier |

### Utility Agents

| Agent | Responsibility |
| --- | --- |
| **debugger** | Applies scientific bug investigation: hypothesis, verification, and root-cause confirmation before fixing |
| **notewriter** | Handles Obsidian writes including private research capture, work diary, inbox, knowledge extraction, and shutdown ritual |

### Domain Agents

Domain agents deliver specialized professional consulting services and can be invoked by users or commands at any workflow stage.

| Agent | Responsibility |
| --- | --- |
| **architect** | Conducts adversarial plan reviews: analyzes trade-offs, detects over-engineering, assesses bug surface area, and evaluates public API risk |
| **analyst** | Reviews business logic: evaluates ROI, validates domain correctness, and assesses user impact |
| **designer** | Creates design systems, explores visual options, builds design-to-code implementations, and audits live UI |
| **researcher** | Performs local-first research with structured synthesis and comprehensive source attribution |
| **security** | Executes OWASP and STRIDE security reviews during the implementation stage |
| **releaser** | Prepares releases, orchestrates deployments, and synchronizes documentation |

### Pipeline Agents

The pipeline represents GAL's automated execution core. It maintains a fixed four-agent main chain, but when implemented changes affect security-sensitive areas, `/gal pipeline` automatically inserts a conditional `golem-security` review before task completion.

```text
T-NNN ──> implementer ──> tester ──> reviewer ──> [conditional security] ──> git commit ──> T-NNN+1
               ↑                         │
               │                         ↓
     auto-fix by review result <────── REJECT

After all tasks are completed: ──> verifier ──> Confirm plan goals are met
```

`[conditional security]` activates `golem-security` only when changes affect authentication, sensitive data handling, input validation, public API interfaces, or deployment/environment trust boundaries.

| Agent | Responsibility | Key Rule |
| --- | --- | --- |
| **implementer** | Completes implementation according to the plan and current `T-NNN` task | Must stop and return to `/deep-planning` if architectural boundaries are reached or the plan proves insufficient |
| **tester** | Writes or supplements tests based on specifications and public APIs, running browser QA when necessary | Operates in spec mode without reading implementation code, and must use a different model than the implementer |
| **reviewer** | Reviews change diffs, identifies risks, and verifies completeness to senior engineer standards | Must return code to the implementer for fixes if blocking issues are found. Uses a different AI model—no weaker than the implementer |
| **verifier** | After all tasks complete, confirms whether the plan's goals were actually achieved through reverse verification | Responsible for determining plan closure eligibility and extracting valuable knowledge back into `docs/` |

`golem-security` belongs to domain agents and is conditionally activated by `/gal pipeline` during security-sensitive changes—it does not participate in normal execution. Domain and utility agents can be invoked directly at any time, and pipeline agents can also be called directly for clearly scoped work.

### AI Model and Agent Rules

In the Pipeline workflow, GAL enforces the use of different models for review and testing:

- The tester **must** use a different model from the implementer
- The reviewer **should** be different from the implementer, and should have capabilities at least as strong as the implementer
- The planning author and architect **should ideally** use different models

These rules are configured in `model-roles.local.md`.

### Working Hours

Working Hours is now an **opt-in** local machine setting. Agents only perform reminders and work stoppage according to configured work hours, After Hours, Wrap-up Time, and Hard Stop when the user enables it in `config.local.env`.

- **Working Hours off**: All agents work normally
- **After Hours**: After the work period ends, work can still continue before Wrap-up Time
- **Wrap-up Time**: If the daily diary has not been written, non-`notewriter` agents block and guide the user into the shutdown ritual
- **Hard Stop**: All agents stop working, including `notewriter`
- **Override**: Users can say `override working hours`, valid for a single time

You can change `WORKING_HOURS_ENABLED`, `WORKDAY_START`, `WORKDAY_END`, `WRAP_UP_TIME`, and `HARD_STOP_TIME` in `config.local.env` to set the activation time. For details, see [docs/personalization.md](docs/personalization.md).

## Storage Boundaries

GAL divides persistent data into two boundaries:

- **Repo shared state**: `.dev/`, `docs/plans/`, `docs/research/`. These files are managed by Git and are suitable for work results that need to be tracked, reviewed, and collaborated on together with the Repo.
- **User private note repository**: Obsidian Vault. Its location is configured by `OBSIDIAN_VAULT` and `OBSIDIAN_VAULT_NAME` in `config.local.env`, and detailed paths can be specified via `OBSIDIAN_PRIVATE_RESEARCH_DIR`, `OBSIDIAN_DIARY_DIR`, and `OBSIDIAN_ARCHIVE_DIR`.

If the user has set `OBSIDIAN_GUIDE_PATH` and the Guide exists, `notewriter` will work according to that Guide. If it is not set or cannot be found, it defaults to generic mode instead of aborting due to a missing Guide.

## Project Files

| Path | Purpose |
| --- | --- |
| `.dev/project.md` | Repo summary, Tech Stack, goals, constraints |
| `.dev/state.md` | Active plan index, blockers, session continuity |
| `.dev/plans/<plan-slug>.prompt.md` | AI execution work file |
| `CLAUDE.md` | Repo local operation notes |
| `DESIGN.md` | Repo-level design governance |
| `docs/designs/<plan-slug>/` | Design assets tied to the plan |
| `docs/plans/<plan-slug>.md` | Human-readable plan document |
| `docs/research/` | Repo shared research reports (default research output) |

### Execution Work File

After you run `/plan-to-prompt`, GAL creates `.dev/plans/<plan-slug>.prompt.md`. You can think of it as the working board for that task. In normal use, the two most important things to read first are:

- `## Status > Workflow`: which phase the work is in right now
- The write-back sections: where planning, testing, review, and handoff results are recorded

Normally you do not need to edit `Workflow:` by hand. It is initialized as `DRAFT` when the execution work file is created and then moves forward automatically as work progresses.

#### Workflow States

| State | Meaning |
| --- | --- |
| `DRAFT` | The execution work file has just been created and execution has not started |
| `IMPLEMENT` | Implementation of a `T-NNN` task is in progress |
| `TEST` | Implementation is done and testing is in progress |
| `REVIEW` | Testing passed and code review is in progress |
| `REVIEW — N blocking issues found` | Review found blocking issues and the work must be fixed before review runs again |
| `ABSORBED` | The goal has been confirmed complete and the plan is ready to close |

#### Write-Back Sections

| Section | When you look at it | Purpose |
| --- | --- | --- |
| `## Open Questions` | When requirements, assumptions, or boundaries are still unresolved | Central list of unresolved issues |
| `## Tasks` | When you need to know what work the plan actually contains | Task list and basis for pipeline execution |
| `## Analyze` | When you want to check whether the current diff still matches the original plan scope | Records reviewer drift analysis |
| `## Review Results` | When you want review outcomes | Collects review results from reviewer, designer, and security specialists |
| `## Test Plan` | Before testing starts, when you want to know what should be verified | Records intended verification scope |
| `## Test Results` | After tests or browser QA are complete | Records test outcomes |
| `### Handoff Notes` | When you pause and need to know where work last stopped | Provides resume Context for the next session |
| `## Release` | When preparing to merge, deploying, or syncing docs | Records release-stage results |

`/gal status` and `/gal whats-next` read these written sections to determine state and next action. They do not rely on one field alone.

## Research Workflow

Research is a workflow separate from development and can run in parallel with development work. The default durable destination remains `docs/research/`, but you can explicitly direct the result to private notes, long-term knowledge capture, or no durable write.

| Mode | Best for | Flow | Source requirement |
| --- | --- | --- | --- |
| `/gal research` | Standard structured investigation | RESEARCH → VERIFY → DOCUMENT | Enough to answer the question |
| `/gal deep-research` | High-risk, high-ambiguity, or cross-topic investigation | RESEARCH → SYNTHESIZE → CROSS-REVIEW → VERIFY → DOCUMENT | Must attempt at least 5 sources |

Both modes require VERIFY to be performed by a **model different from the research author**. CROSS-REVIEW in `deep-research` is a consistency review across sources, not architecture or business review. The type of gap determines where the workflow falls back:

```text
IDLE → RESEARCH → SYNTHESIZE → CROSS-REVIEW → VERIFY → DOCUMENT → DONE
         ↑            ↑             ↑             │
         └─ evidence ─┴─ synthesis ─┴─ source/ref ┘
```

The DOCUMENT destination can be one of four choices:

- `repo`: Write to `docs/research/` (default)
- `private`: Write to `OBSIDIAN_PRIVATE_RESEARCH_DIR`
- `knowledge`: Hand off to `notewriter` for reusable long-term knowledge notes
- `none`: Return the verified result without creating a durable artifact

## Collaborative Tools

GAL can also leverage collaborative tools. These tools enhance specific query capabilities or agent skills across the GAL workflow, but none of them are required by `/gal`, so GAL still functions fully even if none of the tools below are installed. You can install them by running `Setup-Tools`.

### Shared Preflight Mechanism

All collaborative tools go through the same preflight checking mechanism before use:

```text
applicability → availability → initialization status → readiness → route / degrade
```

| State | Meaning |
| --- | --- |
| `not-applicable` | The current workflow or task does not need this tool — skip silently |
| `unavailable` | The machine cannot access the tool, or it cannot run — use the fallback path |
| `available-but-needs-init` | The tool exists but first-time setup is incomplete — do not auto-initialize during normal flow |
| `available-but-not-ready` | Installed and initialized, but the current Repo or task lacks the required artifacts |
| `ready` | Applicable and all preconditions satisfied — route into the tool |

Core rules: never auto-install, never auto-initialize, never hide missing capabilities behind vague success language. Every tool-enabled flow has an explicit degrade path. Full spec in [docs/collaborative-tools/checking-contract.md](docs/collaborative-tools/checking-contract.md).

### graphify

A graph data structure tool. It analyzes all files in a folder into a graph format, writing the output to `graphify-out/`, enhancing subsequent AI query capabilities. GAL only consumes graphify when the repo already contains artifacts such as `graphify-out/GRAPH_REPORT.md`. `gal init` does not generate them automatically. `setup-tools`, `/gal status`, and `/gal whats-next` can report whether an existing stamped report still matches the installed graphify version, but GAL continues through the normal non-graph workflow when no graphify artifacts exist. See [docs/collaborative-tools/graphify.md](docs/collaborative-tools/graphify.md).

### OpenCLI

Turns websites, browser sessions, Electron apps, and local tools into a command-line interface (CLI). You can reuse logged-in browsers, automate live operational flows, and crystallize repeated actions into reusable CLI commands. See [docs/collaborative-tools/opencli.md](docs/collaborative-tools/opencli.md).

### gstack

Created by Garry Tan, President & CEO of Y Combinator, transforming his startup experience into AI agents. See [docs/collaborative-tools/gstack.md](docs/collaborative-tools/gstack.md).

### xmachine

xmachine is GAL's collaborative execution tool for routing bounded work over SSH to readied work nodes. Its documented lanes include Windows work-node dispatch and POSIX-compatible shell work-node detached execution. See [docs/collaborative-tools/xmachine.md](docs/collaborative-tools/xmachine.md).

### Godot C Sharp

Existing commands operate on Godot 4 C# Repos through conventions, skills, and MCP tools. See [docs/collaborative-tools/godot.md](docs/collaborative-tools/godot.md).

### AI-First Game Assets

Uses ComfyUI as the generation entry point, paired with downstream tools for organization and export. See [docs/collaborative-tools/graphworkflow.md](docs/collaborative-tools/graphworkflow.md).

## Personalization

Everything related to the local machine environment that does not belong on the README front page is collected in [docs/personalization.md](docs/personalization.md). It covers how to fill environment placeholders, how to choose and reconfigure execution environments, model routing, MCP overrides, Obsidian Vault paths, private research directories, the optional Guide path, Working Hours settings, and when setup needs to be rerun. If you need to adjust the AI tools you use locally, model role mappings, or MCP settings, please see this document.

## Documentation

`docs/` is mainly for fast human reading and lookup. `docs/collaborative-tools/` is the quick-start and index layer for collaborative tools and workflow guides.

| Path | Purpose |
| --- | --- |
| [docs/devguide.md](docs/devguide.md) | Developer guide |
| [docs/personalization.md](docs/personalization.md) | Local model routing, MCP overrides, and other personalization guidance |
| [docs/collaborative-tools/checking-contract.md](docs/collaborative-tools/checking-contract.md) | Shared preflight checking contract for collaborative tools |
| [docs/collaborative-tools/graphify.md](docs/collaborative-tools/graphify.md) | Graph-structured analysis tool |
| [docs/collaborative-tools/opencli.md](docs/collaborative-tools/opencli.md) | OpenCLI guide and usage scenarios |
| [docs/collaborative-tools/gstack.md](docs/collaborative-tools/gstack.md) | gstack collaborative tool contract: integration of planning and expert agents |
| [docs/collaborative-tools/xmachine.md](docs/collaborative-tools/xmachine.md) | xmachine Execution Lanes, ownership model, smoke testing, and patch-first convergence |
| [docs/collaborative-tools/godot.md](docs/collaborative-tools/godot.md) | Godot C# workflow guide |
| [docs/collaborative-tools/graphworkflow.md](docs/collaborative-tools/graphworkflow.md) | AI-first game asset workflow guide |

## References

- [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done)
- [GitHub Spec Kit](https://github.com/github/spec-kit)
- [gstack](https://github.com/garrytan/gstack)
- [rtk](https://github.com/rtk-ai/rtk): rtk filters and compresses command outputs before they reach LLM context. Strongly recommand to install it.

## License

MIT — See [LICENSE](LICENSE).
