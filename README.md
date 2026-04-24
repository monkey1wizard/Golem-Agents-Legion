# Golem Agents Legion (GAL)

English | [繁體中文](README.zh-Hant.md)

GAL is an AI working system designed to make development more structured while allowing you to switch between AI tools without losing the Context Window. Its core consists of 12 clearly separated Golem Agents plus a `/gal` control plane, forming a document-driven development model. Durable state is split across two boundaries: Repo-shared state saved as local Markdown files in `.dev/`, `docs/plans/`, and `docs/research/`. User personal notes can optionally be written to a locally configured Obsidian Vault. This allows GitHub Copilot, Gemini CLI, and Codex CLI to share the same workflow while still preserving the user's own non-Repo note space.

State management draws from the phase-based discipline in Get Shit Done (GSD): explicit state (`.dev/state.md`), verification gates, and a structured execution lifecycle, allowing `/gal status` and `/gal whats-next` to project the current state of work for the entire Repo.

## Quick Start

1. Clone this project.
`git clone https://github.com/monkey1wizard/golem-agents-legion.git`

2. After entering the project directory, run the installation. Use `./scripts/Setup-Machine.ps1` on Windows and `./scripts/setup-machine.sh` on macOS/Linux. Once finished, you can start using it in the Repo.

3. Inside a target Repo, open GitHub Copilot, Gemini CLI, or Codex CLI, then run:

```text
# Copilot / Gemini CLI (slash-command surface)
/gal init

# Codex CLI (skill mention surface, uses $ instead of /)
$gal init
```

## Public Commands

| Command | Purpose |
| --- | --- |
| `/gal init` | Initialize Repo: create `.dev/project.md` and `.dev/state.md` |
| `/gal status` | Full state projection: active plans, review/test status, blockers, continuity |
| `/gal whats-next` | Recommend a single next action |
| `/gal wrap-up` | Converge work: write `### Handoff Notes` and `## Session Continuity` |
| `/gal research` | Enter the research workflow |
| `/gal deep-research` | Enter the multi-source research workflow, including cross-review |
| `/gal pipeline` | Automatically chain implementer → tester → reviewer per task. Insert a conditional `golem-security` review if changes involve a security-sensitive surface, and finally wrap up with the verifier |
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

`/planning` turns new requirements into a formal source plan, writing it to `docs/plans/<plan-slug>.md`.

The focus of this stage is to organize your goals and requirements into a stable human-readable document, and record unresolved items in `## Open Questions`. `/planning` does not create the execution work file. It only decides whether the current plan should first go through `/deep-planning` for further convergence and architecture review, or move down to `/refining-plan` to lock the implementation contract.

### deep-planning

`/deep-planning` performs deep planning on an existing source plan, and the architect will always activate within this command. Planning materials are organized back into the same `docs/plans/<plan-slug>.md`, and the architecture review results are written to `## Review Results > ### Architecture Review` and `## Approval > Architect review` in the plan. If the architecture review still has blocking issues, work remains in deep-planning for further fixes. Only when the plan is sufficiently converged to enter the execution phase does it move down to `/refining-plan`, then `/plan-to-prompt`.

If the plan involves business logic, pricing, permissions, notifications, onboarding, or identity verification, the analyst activates alongside the architect. If it touches customer-facing flows, layout, states, components, or accessibility, the designer also joins. If you only need a specialist review at the business or design level and do not intend to conduct a full architecture review, you can directly enable the corresponding Domain Lane against the plan document.

### refining-plan

`/refining-plan` completes the three key sections that must be finalized before prompt execution: `## Tasks` (listing `T-NNN` tasks), `## Test Plan` (creating `TP-NNN` test matrices corresponding to the tasks), and `## Review Results > ### Engineering Review` (marked as CLEAR (`<!-- ENG_REVIEW: CLEAR -->`) or BLOCKING with an explanation of blocking issues). This step bridges the gap between planning and `/plan-to-prompt` through detailed design. `/refining-plan` does not involve code implementation, test execution, or `## Status` changes. If gstack is installed, its `plan-eng-review` can be used as an alternative.

### plan-to-prompt

`/plan-to-prompt` converts the plan document into `.dev/plans/<plan-slug>.prompt.md` using a template for the execution phase and updates the state. Afterward, you can run `/gal status` or `/gal whats-next`, and the system will scan the execution work file to answer you. Once the execution work file exists, `/gal pipeline` can run normally. If the plan document changes after conversion, `/plan-to-prompt` should be rerun to ensure consistency between the plan document and the execution work file.

## Golem Agents

GAL's core consists of 12 specialized agents, each with an independent `.agent.md` definition. The separation of duties design principles are:

- **Lean prompts**: Each agent only loads its own role definition, avoiding wasting the context window.
- **Independence**: Independent tester / reviewer / verifier to ensure the credibility of verification results.
- **Composability**: Enable agents based on the risk level of the task, not all at once.

In addition to using `/gal` commands, you can also directly call `golem-` to perform specific types of work.

### Classification

| Category | Activation | Members |
| --- | --- | --- |
| **Utility** | Called directly at any time | debugger, notewriter |
| **Domain** | Consulted via command or directly by the user | architect, analyst, designer, researcher, security, releaser |
| **Pipeline** | Automatically chained by `/gal pipeline` | implementer, tester, reviewer, verifier |

### Utility Agents

| Agent | Responsibility |
| --- | --- |
| **debugger** | Scientific bug investigation: hypothesis, verification, root-cause confirmation before fixing |
| **notewriter** | Main entry for Obsidian writes: private research capture, work diary, inbox, knowledge extraction, shutdown ritual |

### Domain Agents

Domain agents provide professional consulting and can be called by the user or command at any stage.

| Agent | Responsibility |
| --- | --- |
| **architect** | Adversarial plan review: trade-off analysis, over-engineering detection, bug surface area, public API risk |
| **analyst** | Business logic review: ROI, Domain correctness, user impact |
| **designer** | Design system creation, visual exploration, design-to-code build, live UI audit |
| **researcher** | Local-first research and structured synthesis, with source attribution |
| **security** | OWASP and STRIDE security review in the implementation stage |
| **releaser** | Release prep, deploy orchestration, document synchronization |

### Pipeline Agents

Pipeline is the automated execution core of GAL. Its fixed main chain remains four agents, but if the implemented changes touch a security-sensitive surface, `/gal pipeline` inserts a conditional `golem-security` review before task closeout.

```text
T-NNN ──> implementer ──> tester ──> reviewer ──> [conditional security] ──> git commit ──> T-NNN+1
               ↑                         │
               │                         ↓
     auto-fix by review result <────── REJECT

After all tasks are completed: ──> verifier ──> Confirm plan goals are met
```

`[conditional security]` means `golem-security` is only activated when changes touch authentication, sensitive data handling, input handling, public API interfaces, or deployment/environment trust boundaries.

| Agent | Responsibility | Key Rule |
| --- | --- | --- |
| **implementer** | Complete implementation according to the plan and current `T-NNN` task | Must stop and return to `/deep-planning` if it hits an architectural boundary or finds the plan insufficient |
| **tester** | Write or supplement tests based on specs and public APIs, running browser QA if necessary | Spec mode does not read the implementation, and must use a different model than the implementer |
| **reviewer** | Review change diffs, risks, and completeness to the standard of a senior engineer | Must return to the implementer for fixes if blocking issues are found. The AI model used should be different from the implementer and not weaker than the implementer |
| **verifier** | After all tasks are completed, reverse-verify whether the results were actually achieved from the plan's goals | Responsible for confirming whether the plan can be closed and extracting valuable knowledge back into `docs/` |

`golem-security` belongs to the domain agents and is conditionally activated by `/gal pipeline` during security-sensitive changes, not participating in normal execution. Domain and utility agents can be called directly at any time, and pipeline agents can also be called directly for clearly defined work.

### AI Model and Agent Rules

In the Pipeline flow, GAL enforces the use of different models for review and testing:

- Tester **must** use a different model from the implementer
- Reviewer **should** be different from the implementer, and its capabilities should not be weaker than the implementer
- Planning and architect **should ideally** use different models

The above rules are configured in `model-roles.local.md`.

### Working Hours

Working Hours is now an **opt-in** local machine setting. Only when the user enables it in `config.local.env` will the agents perform reminders and work stoppage according to the configured work hours, After Hours, Wrap-up Time, and Hard Stop.

- **Working Hours off**: All agents work normally
- **After Hours**: After the work period, work can still be done before the Wrap-up Time
- **Wrap-up Time**: If the daily diary is not written, non-`notewriter` agents will block and guide into the shutdown ritual
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

GAL can also use collaborative tools. These tools strengthen specific query capabilities or agent skills across the GAL workflow, but none of them are required by `/gal`, so GAL still works fully even if none of the tools below are installed. You can install them by running `Setup-Tools`.

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

A graph data structure tool. It analyzes all files in a folder into a graph format, writing the output to `graphify-out/`, enhancing subsequent AI query capabilities. If the graphify CLI is installed, `gal init` now generates `graphify-out/` automatically during Repo bootstrap and stamps the generated report with the current graphify version. GAL does not auto-detect Repo changes, but `setup-tools`, `/gal status`, and `/gal whats-next` can now warn when a stamped report no longer matches the installed graphify version, and `/gal pipeline` plus `/gal wrap-up` will remind you to rerun `/graphify .` after implementation work. See [docs/collaborative-tools/graphify.md](docs/collaborative-tools/graphify.md).

### OpenCLI

Turns websites, browser sessions, Electron apps, and local tools into a command-line interface (CLI). You can reuse logged-in browsers, automate live operational flows, and crystallize repeated actions into reusable CLI commands. See [docs/collaborative-tools/opencli.md](docs/collaborative-tools/opencli.md).

### gstack

Created by Garry Tan, President & CEO of Y Combinator, transforming his startup experience into AI agents. See [docs/collaborative-tools/gstack.md](docs/collaborative-tools/gstack.md).

### xmachine

xmachine is GAL's collaborative execution tool, distributing work to controlled Machine Lanes via SSH. Both controlling and controlled ends must provide SSH, and the controlled end must install Zellij. See [docs/collaborative-tools/xmachine.md](docs/collaborative-tools/xmachine.md).

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
