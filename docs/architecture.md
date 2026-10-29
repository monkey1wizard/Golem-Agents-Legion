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

- **Rust workspace**: Organized as a Cargo workspace containing seven crates (`gal-foundation`, `mcp`, `dispatch`, `pipeline`, `projection`, `gal-engine`, `cli`), compiled into the unified binary `gal`. The `dispatch` crate also builds the headless dispatch binary `gal-dispatch`.
- **Zero external database dependencies**: The runtime operates without relational or document databases. All state is persisted in plain-text files within repositories and local filesystems.
- **Version-controlled source contracts**: Workflow contracts, command specifications, agent role definitions, and templates are tracked as Markdown files under `plugins/gal-core/`.
- **English canonical baseline**: Under `PROJECT_LANGUAGE`, English (`en`) serves as the authoritative source for contracts and primary documentation. Non-English planning artifacts maintain semantic parity with English execution prompts through deterministic hashing and equivalence stamping.

### Deployment Baseline

- **Precompiled cross-platform targets**: Release pipelines produce prebuilt binaries for six target platforms: `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-unknown-linux-musl`, and `aarch64-unknown-linux-musl`.
- **Supply chain verification**: Release assets include SHA-256 checksums in `checksums.txt`. The release workflow signs `checksums.txt` with keyless cosign and publishes `checksums.txt.sig` and `checksums.txt.pem`. Installation scripts always verify the SHA-256 checksum before extraction. They verify the cosign signature on a best-effort basis. When cosign or the signature files are unavailable, they warn and continue. A verification run that completes and fails aborts the install.
- **Package manager distribution**: Users install binaries using official shell scripts (`install.sh`, `install.ps1`), Homebrew taps, WinGet, or `cargo install --git`. Legacy self-install commands (e.g., `crates/setup`) have been retired.
- **Static CRT on Windows**: Windows release binaries statically link the C runtime (CRT), allowing execution on clean installations without requiring Visual C++ redistributables.

### Standing Architectural Constraints

- **Cross-runtime parity**: Control-plane command behaviors remain consistent across Claude Code, OpenAI Codex, GitHub Copilot, Google Antigravity, and opencode, preventing cross-tool drift.
- **Repository-owned state**: The `.dev/` directory, plan files, and research documents serve as the single source of truth for session continuity. Volatile runtime caches must never supersede repository files.
- **Template-driven adapters**: Repository adapter files must be compiled from source templates by the `gal` binary. Direct manual edits to generated files are forbidden.
- **Standard Git history**: Repositories use standard `git merge`, `git commit`, and `git branch` workflows. Squash commits, rebasing, `--ff-only`, and `--no-ff` operations are prohibited. Concurrent tasks run in isolated Git worktrees rather than rewriting history.
- **Scope integrity**: Once the project owner defines goals and scope boundaries, agents may not unilaterally reduce scope, split features into unapproved follow-up phases, or introduce arbitrary retention lists.
- **Strict token and file budgets**: The project index `.dev/project.md` has an advisory budget of 30,720 bytes, and `.dev/state.md` has an advisory budget of 16,384 bytes. `gal finalize-check` reports an overrun in the `project-md-bound` and `state-bound` rows as `advisory`, and GAL repairs it under `plugins/gal-core/conventions/token-budget.md`. In adapter files, workflow obedience markers must appear before byte offset 32,768 (Codex's default `project_doc_max_bytes` boundary), ensuring partial document reads capture critical constraints.

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
| `projection` | File projection engine: projecting skills, commands, instructions, and agents to target surfaces. | Must not generate repository-level adapters (`AGENTS.md`) or manage headless executor processes. |
| `gal-engine` | Workflow CLI primitives: canonical root rendering (`~/.gal/plugins/gal/`), CLI types, doctor checks, Git filter hooks. | Must not project files directly to external tool paths or call `dispatch` or `pipeline`. |
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
│       ├── codex/               Codex hook configuration (hooks.json)
│       ├── hooks/               Git hooks (commit-msg, pre-commit)
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
| Machine-local execution routing | `~/.gal/config/config.json#executorRouting` | User-managed machine settings, including `sshTarget` and `remoteWorkdir`, which apply only to `pipeline` and `research` headless dispatch. |

### Worktree-Local Runtime Lanes

`source-worktree` validation uses an immutable executable under the canonical worktree. `downstream-installed` repositories use the package-manager-installed executable. Only the package manager may install or promote the shared executable. `CoordinatorState.execution_binding` is the only durable binding authority. Trusted receipts carry its digest.

```text
(start) a gal command needs a trusted executable
  │
  ▼
{ which lane? }
  ├── source-worktree ──────▶ [ private generation in target/gal-pipeline/ ] ─┐
  └── downstream-installed ─▶ [ executable the package manager installed ] ───┤
                                                                              │
  ┌───────────────────────────────────────────────────────────────────────────┘
  ▼
[ validate the binding, executable path, hash, and receipt digest ]
  │  @CoordinatorState.execution_binding
  ▼
{ binding valid? }
  ├── missing or mismatch ──▶ (end ×) hard stop
  │
  valid
  ▼
[ run the trusted action ]
  │
  ▼
{ rebuild needed at a safe segment? }
  ├── no  ──▶ (end √) continue with the bound executable
  └── yes ──▶ [ rebind to a new immutable generation, keep previous bytes ]
                │
                ▼
              (end √) continue with the new generation
```

### Pipeline Attempt Ownership

#### Host-Subagent Attempt and Coordinator Binding

Host-subagent execution is host-managed and participates in pipeline authority without claiming that the subagent shares the orchestrator's operating-system process. Each attempt binds the selected host, task and phase, attempt identity, model and tier, completion state, and returned evidence. The orchestrator owns opening and closing the attempt and validating its scoped result.

Under guarded continuation, the host-subagent resume is bound to the coordinator's durable checkpoint publication. The driver emits the exact binding consumed by the `resume_host_subagent` JSON action. Legacy close uses `--close` and `--session`. Neither path permits mutation of an existing run pin.

The pipeline serializes write operations to a canonical prompt using a prompt-writer lease. It acquires this lease before taking an immutable prompt snapshot and preparing mutable phase state. It then acquires a receipt lease, releasing both leases in reverse order only after placing the receipt, capturing terminal evidence, and confirming process cleanup. If process cleanup cannot be confirmed, GAL retains all active leases and recovery artifacts. The pipeline attempt captures validated receipt bytes and their digest under the receipt lease. Phase writeback consumes this immutable snapshot, and final gate checks verify that on-disk receipts match it. GAL never infers ownership from lock age and never automatically takes over an abandoned lease.

Workflow continuation (using `from <task>`) and executable phase or task arguments are distinct interfaces. Malformed or invalid continuation inputs fail before process spawn and never fall back to executing a default task. Dispatched Codex child processes run with `--dangerously-bypass-approvals-and-sandbox`, outside the Codex sandbox, regardless of the parent session's approval mode, because the Codex Windows sandbox currently fails before it runs any command. Availability lookup, process launch, provider responses, terminal execution states, receipt validation, and gate results are tracked as distinct evidence stages. For user-facing Codex configuration and recovery workflows, see [setup.md](setup.md#codex-pipeline-execution-setup). The host contract for waiting on the original session after a tool yields is defined under `Headless Executor Dispatch` in the `gal-pipeline` skill.

#### Guarded Coordinator Ownership

`legacy_interactive` remains the default continuation profile. It preserves the existing Claude Code and OpenCode host-owned watcher and continuation loop, invocation syntax, process exit behavior, projection layout, and `EvidenceContract: v1` receipt, marker, and mutation semantics. The ordinary `gal pipeline <prompt>` command does not select guarded mode.

`codex_stop_v1` is selected only after the trusted Codex hook handshake presents a valid, single-use grant. In this profile, each `gal pipeline` invocation runs one blocking mechanical segment. The Rust coordinator owns task and phase transitions, attempt lifecycle, retry state, typed actions, evidence checks, journal recovery, and bounded status. A segment returns at a typed ORCHESTRATOR checkpoint, a terminal outcome, a human-authority blocker, or an unrecoverable evidence failure.

The coordinator does not make semantic judgments. The same ORCHESTRATOR thread owns task-quality review, boundary-widening judgment, convergence judgment, and goal-backward verification. The coordinator persists a checkpoint whose typed next action is `await_orchestrator`. The ORCHESTRATOR submits a receipt bound to that checkpoint, coordinator revision, prompt or specification hash, and commit, then invokes the typed resume action. A later blocking CLI segment reopens the durable state. GAL does not use a daemon, recursive shell call, hidden supervisor, or in-process wait for model judgment.

Each semantic checkpoint kind has exactly one consuming entry. The guarded `--submit-checkpoint-receipt` entry consumes `task_quality` and `boundary_widening` receipts, and `pipeline-handback-check` consumes the `goal_backward` receipt. A `boundary_widening` checkpoint binds the HEAD at the time it is raised. Its receipt is accepted when that commit is HEAD or an ancestor of HEAD, so the ORCHESTRATOR can first commit the allowlist edit that the Boundary Widening Protocol requires. An accepted receipt makes the coordinator re-run the task boundary check without a new dispatch. A pass advances to test, and a failure blocks with `boundary-widening-recheck-failed`. A rejected receipt blocks with `boundary-violation`. The coordinator never decides that a widening is acceptable.

Retry is a mechanical coordinator transition. When a guarded test or audit verdict fails, the coordinator writes or refreshes one OPEN `Retry Handoff` block in the execution prompt and resolves it when a later verdict of the same origin passes. The guarded `fix` phase dispatches implement fix mode (`--phase implement --fix`) after the retry ceiling check, because the dispatcher accepts no `fix` phase and `--fix` requires that OPEN handoff. A failed fix dispatch records a retry and never advances to test. A resume phase without a handler fails closed before any coordinator write.

A design stop ends the retry path before the retry ceiling. The handback check reports `design-defect` when two completed repair rounds leave an unresolved `fix-regression` or `missing-contract` finding for the same premise and gate, or when a proven fix needs a file outside the task allowlist. The coordinator persists the stop as a blocked action bound to its revision and execution binding. A retry record, a checkpoint receipt, or a restart does not turn it into a runnable action, and an active attempt keeps its custody. The existing security, Protected Path, high or critical, and boundary-scope stops keep their precedence. The ORCHESTRATOR returns to the source plan, and the revised plan must pass prompt-check, preflight, and the execution binding again before any dispatch.

After guarded entry, the coordinator state lives under the existing `.dev/pipeline/<plan-scope>/` authority. Atomic single-file writes bind the prompt and repository, current task and phase, active attempt, retry state, last verified evidence, pending projection transaction, ORCHESTRATOR checkpoint, activation digest, and next action. The coordinator owns only mechanics. The ORCHESTRATOR retains semantic authority, and the host bridge continues the same thread when the Stop hook blocks a premature final response.

#### Process Identity and Dispatch Evidence

Every v2 dispatch persists its intent before process spawn. Its attempt identity binds the host, PID, operating-system process-birth identity, attempt ID, prompt and repository, base commit, and expected phase. GAL waits for or attaches to the exact attempt. It never infers ownership from a PID alone. If ownership is unknown or conflicts with the recorded identity, GAL does not reclaim a lease, kill a process, or redispatch the work.

`EvidenceContract: v2` adds bindings for plan, task, phase, attempt, receipt digest, provider session, prompt or specification digest, and tested commit or diff. The successful provider-session identity and provenance must agree in the dispatch outcome, terminal attempt log, and v2 evidence before downstream gates classify the attempt. The v2 Agy path reads the native session UUID from bounded JSON output produced by the same process or SSH stdout that belongs to the attempt. Missing, malformed, duplicated, mismatched, truncated, or oversized output is a typed evidence failure. A resumed dispatch must return the requested session UUID.

The binary gate evaluates the v2 Three-Condition PASS rule from evidence bound to the same attempt and tested change. Older `completed` evidence cannot validate newer code. Missing or ambiguous v2 evidence does not consume semantic TEST or AUDIT retries and does not trigger automatic redispatch. These additions do not change the v1 receipt, exit-code, public `Dispatch:` marker, CLI, or mutation-order contract.

#### Durable State and Crash Recovery

Before a guarded segment starts a provider or changes a task cursor or progress projection, it recovers any pending projection journal and runs fresh pipeline preflight against the recovered binding. A resume uses a new segment-specific preflight receipt. A missing, stale, failed, malformed, or unwritable receipt stops the segment before provider start or authority mutation. A live owner is attached read-only. An orphaned coordinator resumes from verified durable state only when its activation digest and attempt identity match. Unknown or conflicting ownership stops with a typed conflict.

Task progress across the prompt, source plan, and `.dev/state.md` uses a durable projection journal. The journal records the intended transaction, staged target bytes, and old and new hashes. Recovery applies a replacement only when the target still matches the recorded old or new hash. It completes an interrupted transaction idempotently when those hashes establish ownership. If a target has a third-party hash, recovery stops and preserves that content. The coordinator advances its durable state only after the journal marks the projection committed.

```text
(start) a guarded segment opens the coordinator
  │
  ▼
{ coordinator state exists? }
  ├── no ──▶ [ persist state and dispatch intent before spawn ]
  │            │
  │            ▼
  │          (end √) dispatch starts
  │
  yes
  ▼
{ owner identity? }
  ├── exact live owner ──────────▶ (end >>|) attach and wait, no redispatch
  ├── unknown or conflicting ────▶ (end ×) typed conflict, stop
  │
  matching orphan
  ▼
[ recover the projection journal ]
  │
  ▼
{ journal target hash? }
  ├── third-party hash ──▶ (end ×) preserve target, conflict, stop
  ├── old hash ──────────▶ [ apply staged replacement ] ──────┐
  └── new hash ──────────▶ [ record replacement as complete ] ┤
                                                              │
  ┌───────────────────────────────────────────────────────────┘
  ▼
[ commit journal, then run fresh preflight ]
  │
  ▼
{ preflight receipt passes? }
  ├── no  ──▶ (end ×) stop before provider start
  └── yes ──▶ (end √) advance coordinator state
```

`legacy_interactive` retains its existing host-owned recovery and continuation behavior. It does not require a Codex marker, canary, hook, grant, or coordinator-owned continuation.

## Data Transformation Pipeline

The build pipeline transforms tracked source contracts into runtime projection surfaces and repo-local adapter files.

### Architectural Governance

This architecture document defines module ownership and data flow. Source contracts pass separately into repository adapter generators and canonical root renderers. The projection engine then distributes compiled assets across runtimes. Technical details regarding file layouts, path resolution, transports, and projection registries are governed by [`projection.md`](projection.md). User-configurable settings boundaries are defined in [`configuration.md`](configuration.md).

## Usage Accounting

GAL records the usage that each runtime reports for a dispatched run. It also reports the shared account capacity behind that usage. The single contract is [`usage-accounting.md`](../plugins/gal-core/conventions/usage-accounting.md). This section summarizes the architecture, and the contract controls when the two disagree.

### Evidence Store and Capture

- Evidence is stored under `gal/usage/v1/` in the Git common directory. Pipeline cleanup and linked-worktree teardown leave the store in place, and finalize keeps accounting evidence unchanged.
- Capture runs after each terminal path of a standalone, local, SSH, or host-subagent attempt. A retry or resume creates a new attempt that keeps its session lineage.
- A capture failure leaves the execution outcome, the retry policy, and the receipt unchanged. The report then shows partial or unavailable accounting, with the reason.
- Preflight binds the orchestrator session only when it finds one unambiguous identity. When it finds none, the report shows the identity as unavailable. GAL never selects a conversation automatically.

### Factual Records and Assessments

- Collectors append observations and snapshots. A snapshot names its exact observation set, its reconciliation version, and its rate-set digest.
- Publication never replaces an existing record. Identical content replays unchanged. Different bytes under the same identity fail as a conflict.
- Assessments append classifications only. An assessment names one snapshot and the stable contribution IDs that it classifies.
- An assessment input that contains a factual field fails with exit code 64. Factual fields include prices, token counts, totals, quota fields, and unknown fields.
- No assessment, report, or review rewrites a factual snapshot, a receipt, or a goal verification result. Goal verification stays in the orchestrator process, and no usage command dispatches it.

The usage commands are:

```text
gal usage collect --run <run-id> [--binding <path>] [--rates <path>]
gal usage report --run <run-id> [--snapshot <snapshot-id>] [--assessment <assessment-id>] [--json]
gal usage assess --run <run-id> --snapshot <snapshot-id> --input <path>
```

### Overlapping Metrics and Retries

A parent session can report an inclusive total that already contains its child sessions. GAL counts each child once. In the reconciliation fixture, the parent reports 200 output units that include a child of 60 units. The task total is 200, not 260.

A failed attempt that consumes 100 units and a successful resume that consumes 60 units produce a task total of 160. The outcome of each attempt stays a separate decision.

### Cost, Credits, and Charges

Each estimate keeps three separate values: the provider-reported amount, the rate-derived estimate, and the confirmed charge. GAL never adds these values together.

- A rate-derived estimate needs an owner-supplied rate set. GAL does not fetch rates. A missing or ambiguous rate yields an unknown estimate, never zero.
- Credits, subscription allowances, and currencies are separate units. GAL never converts one into another or into a charge.
- A confirmed charge needs billing evidence in the same scope. A quota change is never a charge.

For example, a report shows `estimate: unknown` when the rate set has no rate for the observed model. The unit totals stay as they are.

### Account Quota Capability Matrix

Quota describes shared account capacity. It is not task consumption, and no flow derives a task cost from it.

| Runtime | Passive source | Control read | Gap behavior |
| --- | --- | --- | --- |
| Codex CLI | None. Stdout carries no quota. | Bounded `codex app-server` RPC read. | A failed read records a gap with its reason. |
| Claude Code | `rate_limit_event` in stream output, when present. | None. | A window missing from the events is unknown, not zero. |
| GitHub Copilot CLI | None. The usage block has no quota snapshot. | None. | The gap is pinned to the tested CLI 1.0.98. |
| Antigravity CLI | Structured JSON quota in the output, when present. | None. | Headless token usage is not account quota. |
| OpenCode | None. | None. | Provider identity is shown with the gap. |

The 2026-10-17 capability audit recorded installed CLI versions. It did not run a paid provider request, and it did not run live quota acceptance for every runtime. OpenCode was not on the PATH during that audit. The contract was published on 2026-10-20.

Native windows keep their type, duration, unit, and direction. GAL does not coerce them into a five-hour and weekly pair. It never treats a missing window as zero or unlimited.

### Fixture Support and Real-Source Acceptance

Fixture tests exercise the parsers, reconciliation, and reports with recorded inputs. Real-source acceptance uses the bounded live harness with an explicitly selected session. It never installs a provider CLI or starts a login. Fixture results do not prove behavior on a real source.

A control read is a no-inference protocol read. Its trace shows the protocol behavior. It does not prove a provider billing guarantee. GAL records the probe method and its duration as metadata, and it never states that a probe cost nothing.

### Trusted Local Operator Limit

Usage evidence assumes a trusted local operator. The store is an ordinary local file store. Integrity and conflict checks detect inconsistent records. They do not prove that no one with write access to the Git common directory edited a record.

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

1. **Repository Adapter Generation** (`crates/cli/src/gal/render.rs`): Extracts the eight required sections from `.dev/project.md` to generate `AGENTS.md`. Does not use `projection`. It calls `gal-engine` only to resolve the embedded source payload.
2. **Canonical Plugin Rendering** (`crates/gal-engine/src/render/`): Merges `plugins/gal-core/` and `~/.gal/local/` into `~/.gal/plugins/gal/`.
3. **Runtime Surface Projection** (`crates/projection/`): Projects assets from the canonical root into tool directories (such as `~/.agents/skills/` or `~/.codex/agents/`).

### Categorization of Golem Agent Roles

Agent personas fall into two structural classes based on invocation capabilities:

- **Class 1: Directly Invocable Roles**
  - Six roles: `golem-architect`, `golem-analyst`, `golem-designer`, `golem-releaser`, `golem-debugger`, and `golem-steward`.
  - Can be invoked directly using `/gal <role>`, running evaluations inside an isolated sandbox to emit structured summaries.
  - Four of them (`architect`, `analyst`, `designer`, `releaser`) also have a standalone command with the same name. Its `discuss` argument starts in-context consultation through the internal `gal consult-script golem-<role>`: `/<role> discuss` in Claude Code, `/gal:<role> discuss` in plugin mode, and `$<role> discuss` in Codex. The removed `/gal discuss <role>` form returns `COMMAND: error` with the replacement. The debugger has no standalone command, and the steward's standalone command has no `discuss`.
  - Isolated direct calls for the four review roles print `ROUTING_SCOPE: codex-native-projection-only`. `planning` configuration feeds only the Codex native-agent projection. It launches no executor, and SSH fields do not apply to it.
- **Class 2: Orchestrator-Driven Roles**
  - Four roles: `golem-implementer`, `golem-tester`, `golem-auditor`, and `golem-researcher`.
  - **Projection constraints**: The projection engine (`update_skills` in `crates/projection/src/lib.rs`, using `ORCHESTRATED_ONLY_GOLEM_NAMES`) is strictly forbidden from creating projection files (`*.agent.md` or `*.toml`) for these four roles.
  - **Fail-closed security**: A direct invocation (e.g., `/gal tester` or `$tester`) returns `COMMAND: error`. Its `ACTION:` line names the role as orchestrated-only and points to `/gal pipeline` or `/gal research`. These roles execute only as headless subprocesses launched by orchestrators like `/gal pipeline`.

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

### Structure Map

`docs/structure/structure-map.ndjson` is the dual-axis structure map. Its code axis tracks files and directories. Its doc axis tracks document sections. Each line is one node with `syncStatus`, `lastSynced`, and `lastSyncedRef` fields. `docs/structure/structure-map.schema.json` defines the line format. The `doc-sync` workflow maintains the map when code changes affect documented behavior.

### Punctuation Policy: Semicolons Prohibited

Documentation in this repository does not use semicolons (half-width `;` or full-width `；`) in any language. Semicolons typically connect multiple thoughts within single sentences. Writers must split independent thoughts into separate sentences or use appropriate coordinating conjunctions. Lists separated by semicolons must be formatted as comma-separated phrases or bulleted lists.

This policy prioritizes readability and structural clarity. It applies to body prose, table cells, diagrams, and code comments. Semicolons required by programming language syntax are exempt.

### Writing-Quality Ownership

For each material factual assertion in the actual unsent draft, the current agent compares the claim with specific available support. It removes or qualifies unsupported claims. Missing completion evidence does not establish that work never started. Artifact-only requests must be honored unless a higher-priority host instruction requires otherwise.

GAL owns portable writing requirements, project terminology, and this repository's optional writing-check integration. `docs/glossary.md` defines semantic names and English lexical choices. The zh-Hant and ja terminology profiles define locale forms and punctuation. The writing convention defines cross-language prose structure and rule ownership. Executable terminology comes only from explicit structured blocks in these authorities, never from inferred prose rules.

The repository checker publishes immutable terminology snapshots at `.dev/cache/writing-terms/<sourceHash>.json` under the document root. It validates and reuses an existing snapshot instead of replacing another writer's data. A child process verifies the parent's source hash against the authorities it actually reads. Direct CLI and official MCP calls resolve their own document root and require an existing matching snapshot. Missing, corrupt, or stale data causes an operational failure. Generating a snapshot remains an explicit prerequisite for those direct routes.

Writers address the actual recipient and supply the background that recipient needs. Facts may come from the request, supplied material, or observed tool results. Writers distinguish those facts from inference, assumptions, suggestions, placeholders, and future commitments. Every human-facing message, including progress, requires an in-process accuracy and readability review. When a configured checker is available, the writer checks the unsent draft. These obligations do not prove that a host intercepts streamed output.

GAL Core carries the neutral minimum and a reference to the full writing convention. It does not copy or require personal `accurate-answer` guidance. Personal guidance remains subordinate to project terminology and mandatory output structures. The minimum still applies when personal guidance is absent or off.

The caller resolves locale from explicit intent, then supported `lang` metadata, then known managed paths. Only managed `docs/i18n/zh-Hant/` documents map `zh-Hant` to `zh-TW`. This does not classify every Traditional Chinese document as Taiwan Chinese. Commands, code, paths, link targets, front matter, machine anchors, IDs, receipts, and literal quotations remain protected.

The wrapper supports `--transport cli` and `--transport mcp`. Both routes produce the same normalized GAL finding and run report. The MCP route uses the official server. When the handshake succeeds, the report records its advertised name and version. Raw textlint CLI/MCP output remains a lower-level diagnostic format, not the complete GAL report.

Compact root and task instructions use the source convention when that file exists. In an unrelated initialized repository, they point to the writing-quality.md section in `~/.gal/plugins/gal/rules/gal.md`, expanding `~` to the user home. This is the existing installed canonical rule file. No private source checkout or personal writing skill is needed. `AGENTS.md` is the only generated repository instruction root. GAL no longer generates `CLAUDE.md`.

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

The `gal translation-freshness` command compares `source_commit` against recent Git commits, reporting files as `current`, `stale`, `missing`, or `unexpected`. The validator is `crates/cli/src/gal/translation.rs`. `cmd_translation_freshness` in `crates/cli/src/commands/system.rs` calls it. Newly drafted translations record `source_commit: PENDING` until canonical English changes are merged.

### Front Matter Metadata Specifications

Managed documentation opens with Open Knowledge Format (OKF) front matter containing five keys. The root and translated READMEs omit OKF front matter so GitHub displays the project introduction first. Each README's description is maintained in its documentation map.

- `type`: Category (`Guide`, `Reference`, `Architecture`, `ADR`, `Policy`).
- `title`: Document title.
- `description`: Single-sentence summary matching `README.md` documentation tables verbatim.
- `tags`: Classification tags.
- `status`: Lifecycle maturity (`draft`, `stable`, `deprecated`).

Other translated documents combine these five keys with the four translation provenance keys, producing nine front matter fields in total. Translated READMEs keep only the four provenance keys in a hidden `gal:translation-metadata` HTML comment immediately after the title, which the translation checks read.

## Architectural Decision Records (ADR) Index

This document aligns with ISO/IEC/IEEE 42010 and arc42 architectural documentation standards. Historical context and rejected alternatives are archived in individual ADR files under `docs/adr/` to keep core architecture documentation focused:

- [01: Projection, Source Model, and Adapters](adr/01-projection-and-source-model.md)
- [02: Scope and Ownership Boundaries](adr/02-scope-and-ownership-boundaries.md)
- [03: Machine Lifecycle and Binary Identity](adr/03-machine-lifecycle-and-binary-identity.md)
- [04: Dispatch Architecture](adr/04-dispatch-architecture.md)
- [05: Gates, Evidence, and Enforcement](adr/05-gates-evidence-and-enforcement.md)
- [06: Documentation and State Governance](adr/06-documentation-and-state-governance.md)
- [07: Release](adr/07-release.md)
- [08: Worktree-Local Runtime Isolation](adr/08-worktree-local-runtime-isolation.md)
