# AI Agent Onboarding

This document explains what Golem Agents Legion (GAL) is, what problem it solves, and how an AI agent should navigate this repository.

## What This Project Is

GAL is a portable, tool-agnostic development methodology system.

It is not a normal application, library, or service. It does not have a runtime product for end users. Instead, this repository is the canonical source for:

- Workflow definitions
- Agent role definitions
- Coding and documentation conventions
- Reusable skills
- Templates for plans, state, and project context
- Setup scripts that install those definitions into AI tooling environments

The core idea is:

> Store methodology in Markdown. Generate tool-specific adapters from it.

That means the durable asset is the methodology itself, not Copilot config, Gemini config, or any specific editor integration.

## What This Project Is Not

An AI agent should not misclassify GAL as any of the following:

- Not a SaaS product
- Not a web app
- Not a language SDK
- Not a backend service
- Not a CLI product whose primary purpose is end-user commands
- Not a repo where success is measured by shipping app features

The scripts and slash commands exist to install and expose the methodology, not because the repo's main goal is command-line tooling.

## Primary Outcome

The main outcome of GAL is consistent AI-assisted development across tools and machines.

GAL tries to make these things portable:

- How work is planned
- How risk is reviewed
- How implementation is constrained
- How testing and verification happen
- How knowledge is captured and reused

## Mental Model

Think of this repo as a methodology operating system for AI-assisted engineering.

It has six major parts:

1. `workflows/`: state machines that define how work moves from idea to done
2. `agent/`: specialized golem roles with clear responsibilities
3. `conventions/`: engineering guardrails and language-specific rules
4. `templates/`: scaffolds for plans, state, project summaries, and diaries
5. `skills/` and `commands/`: reusable AI tool entry points and slash-command surfaces
6. `scripts/`: installation, initialization, and synchronization glue

## How The Repo Fits Into A Real Workflow

GAL participates in a larger workflow with three layers:

1. This repo defines the methodology.
2. Target project repos carry per-project context in `.dev/project.md` and `.dev/state.md`.
3. Tool-specific files such as `copilot-instructions.md` or `GEMINI.md` are generated adapters.

So when working in this repo, an AI agent is usually doing one of these things:

- Evolving the methodology itself
- Refining workflow rules or agent definitions
- Improving setup/install behavior
- Improving tool discoverability and portability
- Writing clearer docs so future agents and humans can use GAL correctly

## Repo Map For AI Agents

### Read These First

If you are new to the repo, read in this order:

1. `README.md` for the high-level architecture and purpose
2. `docs/design-principles.md` for the durable architecture rationale
3. `docs/installation-topology.md` for setup and runtime layout
4. `docs/command-dispatch-architecture.md` for command-surface semantics
5. `docs/per-repo-context.md` for `.dev/`, plans, and working-memory rules
6. `workflows/coding.md` for the primary state machine and tier model
7. `agent/agents.md` for the golem classification and responsibilities
8. `model-roles.md` for role-to-model routing
9. `scripts/scripts.md` and `commands/commands.md` for current operational behavior

### Core Directories

| Path | Why it matters |
| --- | --- |
| `workflows/` | Defines allowed workflow states and transitions |
| `agent/` | Defines what each golem is responsible for and what it must not do |
| `conventions/` | Holds architecture, language, and process rules that implementations should follow |
| `templates/` | Defines the canonical shape of plans, state, and project summaries |
| `skills/` | Canonical skill source reused across tools |
| `commands/` | Slash-command entry points such as `/gal` and `gal-*` aliases |
| `scripts/` | Machine setup, repo bootstrap, and dispatcher logic |
| `docs/plans/` | Temporary execution memory for methodology changes inside this repo |

The long-lived architecture rationale belongs in `docs/`, not in `docs/plans/`.

## Command Surface: How To Think About It

GAL currently uses:

- A canonical `/gal` entry point
- Lightweight `gal-*` aliases such as `/gal-plan` and `/gal-status` for discoverability

An AI agent should understand that the aliases are UX affordances, not independent logic branches. The canonical behavior still flows through the dispatcher.

## How Plans Work Here

This repo follows its own methodology.

That means changes to GAL itself may be tracked in `docs/plans/*.prompt.md`. These plan files are temporary task memory, not permanent product documentation.

When reading a plan in this repo, interpret it as:

- A record of a methodology change being executed
- A source of historical design decisions for that task
- Potentially stale if the repo implementation has moved on

If a plan conflicts with current source files, prefer current source files and treat the plan as historical unless explicitly refreshed.

## Common Misreadings To Avoid

### Mistaking historical plans for current architecture

Plans in `docs/plans/` can become stale after implementation changes. They are useful context, but they are not automatically the current truth.

### Treating GAL as a pure prompt library

GAL is broader than prompt files. It includes workflows, roles, conventions, installation topology, and adapter generation.

### Treating slash commands as the product

The slash commands are only the user-facing entry points into the methodology. They are not the methodology itself.

### Optimizing for one tool at the expense of portability

A change that is elegant for one tool but breaks portability across Copilot and Gemini is usually architecturally wrong for this repo.

## Design Priorities

When changing this repo, favor these priorities:

1. Portability across AI tools
2. Single-source-of-truth documentation
3. Low duplication in behavior and logic
4. Strong workflow guardrails
5. Clear separation between canonical methodology and generated adapters
6. Discoverability for humans and AI agents

## How An AI Agent Should Start A Task In This Repo

Before editing, establish which category the task belongs to:

- Workflow change
- Agent definition change
- Convention change
- Skill or slash-command change
- Setup/install topology change
- Documentation clarification

Then read only the relevant files for that category instead of scanning the whole repo.

### Typical file entry points by task

| Task type | Start here |
| --- | --- |
| Workflow behavior | `workflows/coding.md` |
| Agent responsibilities | `agent/agents.md` plus the specific `.agent.md` file |
| Command behavior | `commands/commands.md`, `commands/gal/`, `scripts/gal.ps1`, `scripts/gal.sh` |
| Installation behavior | `scripts/Setup-Machine.ps1`, `scripts/setup-machine.sh`, `scripts/scripts.md` |
| Architecture rationale | `docs/design-principles.md`, `docs/installation-topology.md`, `docs/command-dispatch-architecture.md` |
| Per-repo context model | `docs/per-repo-context.md`, `templates/project.md`, `templates/state.md`, `templates/plan.md` |
| Project purpose / orientation | `README.md`, this file |

## Short Summary

If you only remember one thing, remember this:

GAL is a methodology repo that teaches AI tools how to plan, review, implement, test, verify, and document engineering work in a portable way.

The repo's purpose is not to ship an app. Its purpose is to make AI-assisted engineering consistent and transferable.
