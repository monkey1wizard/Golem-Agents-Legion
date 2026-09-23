---
type: Architecture
title: GAL Architecture
description: Details GAL architectural constraints, crate and layer boundaries, storage topology, migration paths, documentation governance, and the ADR index.
tags:
  - architecture
  - boundaries
  - crates
  - governance
  - arc42
status: stable
---

# Architecture

## Technical Baselines

GAL is a cross-runtime developer workflow engine built in Rust, maintaining a unified control plane and standardized workflow contracts across multiple AI providers. The technical foundation comprises the source baseline, the deployment baseline, and architectural standing constraints.

### Source Baseline

- **Rust workspace**: Organized as a Cargo workspace containing seven crates (`gal-foundation`, `mcp`, `dispatch`, `pipeline`, `projection`, `gal-engine`, `cli`), compiled into a single unified binary named `gal`.
- **Zero external database dependencies**: The runtime operates without relational or document databases. All state is persisted in plain-text files within repositories and local filesystems.
- **Version-controlled source contracts**: Workflow contracts, command specifications, agent role definitions, and templates are tracked as Markdown files under `plugins/gal-core/`.
- **English canonical baseline**: Under `PROJECT_LANGUAGE`, English (`en`) serves as the authoritative source for contracts and primary documentation. Non-English planning artifacts maintain semantic parity with English execution prompts through deterministic hashing and equivalence stamping.

### Deployment Baseline

- **Precompiled cross-platform targets**: Release pipelines produce prebuilt binaries for six target platforms: `x86_64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, and `x86_64-pc-windows-msvc`.
- **Supply chain verification**: Release archives are published with SHA-256 checksums (`checksums.txt`) and keyless cosign signatures. Installation scripts verify checksums and signatures prior to extraction.
- **Package manager distribution**: Users install binaries using official shell scripts (`install.sh`, `install.ps1`), Homebrew taps, WinGet, or `cargo install --git`. Legacy self-install commands (e.g., `crates/setup`) have been retired.
- **Static CRT on Windows**: Windows release binaries statically link the C runtime (CRT), allowing execution on clean installations without requiring Visual C++ redistributables.

### Standing Architectural Constraints

- **Cross-runtime parity**: Control-plane command behaviors remain consistent across Claude Code, OpenAI Codex, GitHub Copilot, Google Antigravity, and opencode, preventing cross-tool drift.
- **Repository-owned state**: The `.dev/` directory, plan files, and research documents serve as the single source of truth for session continuity. Volatile runtime caches must never supersede repository files.
- **Template-driven adapters**: Repository adapter files must be compiled from source templates by the `gal` binary. Direct manual edits to generated files are forbidden.
- **Standard Git history**: Repositories use standard `git merge`, `git commit`, and `git branch` workflows. Squash commits, rebasing, `--ff-only`, and `--no-ff` operations are prohibited. Concurrent tasks run in isolated Git worktrees rather than rewriting history.
- **Scope integrity**: Once the project owner defines goals and scope boundaries, agents may not unilaterally reduce scope, split features into unapproved follow-up phases, or introduce arbitrary retention lists.
- **Strict token and file budgets**: The project index `.dev/project.md` is limited to 30,720 bytes, and `.dev/state.md` is limited to 16,384 bytes. In adapter files, workflow obedience markers must appear before byte offset 32,768 (Codex's default `project_doc_max_bytes` boundary), ensuring partial document reads capture critical constraints.

## Directory Boundaries and Dependency Law

The codebase enforces strict module isolation and a downward-only dependency hierarchy, preventing circular references.

### Crate Dependency Graph

Dependencies between crates form a strict directed acyclic graph pointing strictly downward:

```text
gal-foundation  (Foundation layer: zero internal gal dependencies)
dispatch        (Depends exclusively on gal-foundation)

dispatch   ─> gal-foundation
mcp        ─> gal-foundation
projection ─> gal-foundation
pipeline   ─> dispatch
gal-engine ─> gal-foundation, projection
cli        ─> gal-foundation, mcp, projection, dispatch, pipeline, gal-engine (Binary entry point)
```

### Crate Responsibility Matrix (Negative Boundaries)

| Crate | Responsibilities | Forbidden Operations |
| --- | --- | --- |
| `gal-foundation` | Shared foundations: config structures, path utilities, OS abstractions, runtime registries (`VALID_RUNTIMES`), MCP schemas, the `HealthCheck` trait, and `secret_re`. | Must not depend on other GAL crates or implement CLI commands. |
| `mcp` | MCP artifact management: health checks for canonical `.mcp.json`. | Must not inspect, edit, or write to host-managed configuration files such as `claude_desktop_config.json`. |
| `dispatch` | Headless execution: role routing (`executorRouting`), process timeouts, write-back validation, and SSH execution lanes. | Must not manage multi-phase workflow orchestration (managed by `pipeline`) or depend on crates other than `gal-foundation`. |
| `pipeline` | Workflow orchestration: task decomposition, multi-phase dispatching, and task specification assembly. | Must not call OS process-spawning APIs directly or project runtime files. |
| `projection` | File projection engine: projecting skills, commands, instructions, and agents to target surfaces. | Must not generate repository-level adapters (`AGENTS.md`, `CLAUDE.md`) or manage headless executor processes. |
| `gal-engine` | Workflow CLI primitives: canonical root rendering (`~/.gal/plugins/gal/`), CLI types, doctor checks, Git filter hooks, and translation validators. | Must not project files directly to external tool paths or call `dispatch` or `pipeline`. |
| `cli` | Executable entry point: argument parsing, `gal doctor` aggregation, adapter generation (`render.rs`), and internal verification gates. | Must not bypass preflight validation or access internal crate structures across boundaries. |

### Repository Structure and Protected Paths

```text
Golem-Agents-Legion/
├── plugins/
│   └── gal-core/                Canonical source contracts
│       ├── commands/            Public /gal command templates [Protected]
│       ├── conventions/         Shared rules and engineering conventions [Protected]
│       ├── workflows/           Workflow contracts (coding, doc-sync, research) [Protected]
│       ├── templates/           Repository state templates [Protected]
│       ├── agents/              Golem agent contracts (*.agent.md)
│       ├── skills/              Reusable skills (skills/<name>/SKILL.md)
│       ├── mcp.json             Version-controlled MCP manifest
│       └── opencode.json        Bundled opencode configuration
├── crates/                      Rust workspace (7 crates) [Protected*]
├── packaging/                   Installation scripts and mirror exports
├── docs/                        Documentation, guides, and architecture decision records (docs/adr/)
└── .dev/                        Repository operational state (project.md, state.md, plans/, research/)
```

\* Protected paths include `crates/projection/`, `crates/gal-engine/src/render/`, and core contracts in `plugins/gal-core/`. Changes to protected paths represent significant architectural modifications requiring approved plans before implementation.

### Codebase Ownership Matrix

| Artifact Type | Storage Location | Owning Component |
| --- | --- | --- |
| Methodology and core rules | `plugins/gal-core/` | Source contracts (`commands/`, `conventions/`, `workflows/`, `templates/`, `agents/`). |
| Project operational context | `.dev/project.md` and `.dev/state.md` | Target repository local state, maintained via `gal init` and workflow commands. |
| Human-readable feature plans | `.dev/plans/<slug>.md` | Planning artifacts, co-authored via planning commands and architect reviews. |
| Machine execution prompts | `.dev/plans/<slug>.prompt.md` | Generated execution prompt serving as the mutable memory for `/gal pipeline`. |
| Session handoff notes | `### Handoff Notes` in prompts and `.dev/state.md` | Transient notes consolidated into durable documentation during finalization. |
| Machine-local execution routing | `~/.gal/config/config.json#executorRouting` | User-managed machine settings, including `sshTarget` and `remoteWorkdir`. |

## Data Transformation Pipeline

The build pipeline transforms tracked source contracts into runtime projection surfaces and repo-local adapter files.

### Architectural Governance

This architecture document defines module ownership and data flow. Source contracts pass separately into repository adapter generators and canonical root renderers. The projection engine then distributes compiled assets across runtimes. Technical details regarding file layouts, path resolution, transports, and projection registries are governed by [`projection.md`](projection.md). User-configurable settings boundaries are defined in [`configuration.md`](configuration.md).

## Component Model

Internal components adhere to single-responsibility separation and unambiguous terminology.

### Terminology Rules

Terms follow the authoritative glossary in [`docs/glossary.md`](glossary.md):

- Use `projection` for the file distribution engine, replacing ambiguous uses of "adapters".
- Maintain external runtime logic strictly within `projection`.
- Use `executors` for headless CLI runners, replacing legacy references to `dispatch::adapters`.
- Delete deprecated components completely without leaving unused stubs (e.g., the removal of `crates/xmachine`).
- Never include ephemeral task identifiers (`T-NN`, `R-NN`) or migration narratives within persistent documentation.

### Separation of Adapter Generation and Projection

Adapter generation, canonical plugin rendering, and runtime surface projection operate across three independent modules:

1. **Repository Adapter Generation** (`crates/cli/src/gal/render.rs`): Extracts the eight required sections from `.dev/project.md` to generate `AGENTS.md` and `CLAUDE.md`. Does not invoke `gal-engine` or `projection`.
2. **Canonical Plugin Rendering** (`crates/gal-engine/src/render/`): Merges `plugins/gal-core/` and `~/.gal/local/` into `~/.gal/plugins/gal/`.
3. **Runtime Surface Projection** (`crates/projection/`): Projects assets from the canonical root into tool directories (such as `~/.agents/skills/` or `~/.codex/agents/`).

### Categorization of Golem Agent Roles

Agent personas fall into two structural classes based on invocation capabilities:

- **Class 1: Directly Invocable and Consultative Roles**
  - Six roles: `golem-architect`, `golem-analyst`, `golem-designer`, `golem-releaser`, `golem-debugger`, and `golem-steward`.
  - Can be invoked directly using `/gal <role>`, running evaluations inside an isolated sandbox to emit structured summaries.
  - Review roles also support conversational interaction via `/gal discuss <role>`, injecting role instructions directly into the active session.
- **Class 2: Orchestrator-Driven Roles**
  - Four roles: `golem-implementer`, `golem-tester`, `golem-auditor`, and `golem-researcher`.
  - **Projection constraints**: The projection engine (`update_agents` in `crates/projection/src/lib.rs`) is strictly forbidden from creating projection files (`*.agent.md` or `*.toml`) for these four roles.
  - **Fail-closed security**: Attempting direct invocations (e.g., `/gal tester` or `$tester`) returns an unknown-command error. These roles execute only as headless subprocesses launched by orchestrators like `/gal pipeline`.

## System Boundaries and Integrations

GAL coordinates tool runtimes across clearly defined boundaries. This document specifies structural divisions among contracts, canonical renderers, projections, and repository adapters. Concrete transport mechanisms and file paths are detailed in [`projection.md`](projection.md). Settings schemas are documented in [`configuration.md`](configuration.md). Optional third-party integrations are specified in [`integrations.md`](integrations.md).

## Evolution and Migration Policies

Architectural guardrails ensure backward compatibility and smooth upgrades.

### Compatibility Patterns

- **Serde field aliasing**: When configuration keys or internal attributes are renamed, `#[serde(alias = "...")]` annotations preserve compatibility with existing `config.json` or `plugins.lock.json` files.
- **Safe directory cleanup**: Stale directories are removed only when they carry explicit GAL ownership markers and are empty. Folders containing user files trigger non-blocking warnings and are preserved.
- **Golden test updates**: When template outputs or contract formats change intentionally, corresponding test suites and golden test fixtures must be updated in the same commit with documented rationale.

### Escalation Thresholds

Halt implementation and request an architect review via `/deep-planning` when tasks require:

- Creating or removing root configuration files (e.g., `Cargo.toml`, `package.json`).
- Adding or removing external dependencies.
- Moving files across architectural layer boundaries.
- Introducing new global interfaces or abstract base types.
- Modifying public API signatures or shared module interfaces used by multiple consumers.
- Modifying core abstractions shared by three or more components.
- Editing files within Protected Paths.

### Persistent Knowledge Core

The following assets constitute the permanent knowledge base of the repository and must be preserved across sessions:

- **Source contracts**: Commands, conventions, workflows, and templates under `plugins/gal-core/`.
- **Terminology authorities**: Canonical terminology in [`docs/glossary.md`](glossary.md) and locale profiles in [`docs/i18n/zh-Hant/terminology.zh-Hant.md`](i18n/zh-Hant/terminology.zh-Hant.md).
- **Documentation and ADRs**: Topic guides under `docs/` and decision records under `docs/adr/`.
- **Project specification**: The `.dev/project.md` manifest containing high-density project metadata.

## Documentation Governance

### Durable Knowledge Boundaries

The permanent knowledge base resides exclusively in two locations: the three root documents (`README.md`, `SECURITY.md`, `CONTRIBUTING.md`) and the canonical `docs/` directory.

Directories such as `.dev/plans/` (temporary feature plans) and `.dev/research/` (exploratory research notes) fall outside the permanent knowledge base. Facts discovered during development become durable only when integrated into `docs/` or formalized in plans. The `.dev/project.md` file acts as a compressed index referencing `docs/` rather than storing unlinked facts.

### Punctuation Policy: Semicolons Prohibited

Documentation in this repository does not use semicolons (half-width `;` or full-width `；`) in any language. Semicolons typically connect multiple thoughts within single sentences. Writers must split independent thoughts into separate sentences or use appropriate coordinating conjunctions. Lists separated by semicolons must be formatted as comma-separated phrases or bulleted lists.

This policy prioritizes readability and structural clarity. It applies to body prose, table cells, diagrams, and code comments. Semicolons required by programming language syntax are exempt.

### File Naming Conventions

- `README.md` is reserved exclusively for the repository root. Subdirectories requiring index documents use `guide.md` for curated overviews or `index.md` for structured listings.
- Tool sections in `docs/integrations.md` use official product names. Only sections dedicated to MCP servers include the `MCP` suffix (e.g., "Playwright MCP").
- Documents corresponding to specific skills share names with those skills. For example, the `opencli-research` skill aligns with the OpenCLI section of `docs/integrations.md`.
- Unrelated documents must not share identical file basenames.

### Multilingual Translation Architecture

Canonical documentation is written in English at standard file paths. Translated files reside under `docs/i18n/<lang>/` using the format `<name>.<lang>.md`. Root documents mirror this convention (e.g., `README.md` maps to `docs/i18n/zh-Hant/README.zh-Hant.md`).

Translations cover a closed set of published documents: `README`, `SECURITY`, `CONTRIBUTING`, `architecture`, `configuration`, `workflows`, `projection`, `integrations`, and `setup`. Decision records in `docs/adr/` are English-only and have no translation tree. The English naming authority is represented in locales via `terminology.<lang>.md` presentation profiles rather than full-text translations.

Translations alter prose only. Commands, code, paths, config keys, and identifiers remain untranslated. Relative links are recalculated relative to locale directory depths.

### Translation Freshness Tracking

Translations carry four provenance keys alongside Open Knowledge Format metadata:

```yaml
---
source: README.md          # Relative path to English canonical source
lang: zh-Hant
source_commit: <hash>      # Commit hash of English source, set to PENDING during drafting
translated_at: 2026-06-02  # Date of last translation update
---
```

Freshness is evaluated via `source_commit` rather than `status`. The `status` field represents lifecycle maturity (`draft`, `stable`, `deprecated`) and must not be used to indicate translation freshness.

The `gal translation-freshness` command compares `source_commit` against recent Git commits, reporting files as `current`, `stale`, `missing`, or `unexpected`. Newly drafted translations record `source_commit: PENDING` until canonical English changes are merged.

### Front Matter Metadata Specifications

Managed documentation opens with Open Knowledge Format (OKF) front matter containing five keys:

- `type`: Category (`Guide`, `Reference`, `Architecture`, `ADR`, `Policy`).
- `title`: Document title.
- `description`: Single-sentence summary matching `README.md` documentation tables verbatim.
- `tags`: Classification tags.
- `status`: Lifecycle maturity (`draft`, `stable`, `deprecated`).

Translated documents combine these five keys with the four translation provenance keys, producing nine front matter fields in total.

## Architectural Decision Records (ADR) Index

This document aligns with ISO/IEC/IEEE 42010 and arc42 architectural documentation standards. Historical context and rejected alternatives are archived in individual ADR files under `docs/adr/` to keep core architecture documentation focused:

- [01: Projection, Source Model, and Adapters](adr/01-projection-and-source-model.md)
- [02: Scope and Ownership Boundaries](adr/02-scope-and-ownership-boundaries.md)
- [03: Machine Lifecycle and Binary Identity](adr/03-machine-lifecycle-and-binary-identity.md)
- [04: Dispatch Architecture](adr/04-dispatch-architecture.md)
- [05: Gates, Evidence, and Enforcement](adr/05-gates-evidence-and-enforcement.md)
- [06: Documentation and State Governance](adr/06-documentation-and-state-governance.md)
- [07: Release](adr/07-release.md)
