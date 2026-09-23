---
type: Guide
title: Workflows and Helpers
description: Explains golem agent roles, invocation methods, planning workflows, automated pipeline execution, finalization, recovery, and authoring helpers.
tags:
  - workflow
  - golems
  - pipeline
  - finalize
  - invocation
status: stable
---

# Workflows and Helpers

## Golem Roles and Invocation

### Overview of Golem Agents

GAL organizes specialized AI agents into distinct personas called golems. Each golem carries specific responsibilities, operational scopes, and tool integrations. The canonical agent definitions reside in [`plugins/gal-core/agents/agents.md`](../plugins/gal-core/agents/agents.md), and formal workflow contracts are defined in [`plugins/gal-core/workflows/coding.md`](../plugins/gal-core/workflows/coding.md).

### Capability Matrix

| Golem | Primary Responsibilities | Common Invocations |
| --- | --- | --- |
| `golem-architect` | Adversarial design review evaluating trade-offs, over-engineering, latent bug vectors, and API/dependency risks. | Stress-test proposals during `/deep-planning` or run `/gal architect`. |
| `golem-analyst` | Business-logic analysis verifying ROI, domain consistency, and end-user impact. | Run when changes alter pricing, authorization, billing, or user-facing rules. |
| `golem-designer` | Design systems, UX/UI layouts, accessibility guidelines, and live frontend audits. | Run when adjusting UI components, user interactions, or developer experience (DevEx). |
| `golem-researcher` | Local-first codebase research, multi-source synthesis, and evidence-based investigation. | Use `/gal research` or `/gal deep-research` to investigate unfamiliar topics. |
| `golem-implementer` | Implements approved task specifications with atomic Git commits. | Operates during the `CODER` phase of `/gal pipeline`. |
| `golem-tester` | Derives test cases from plan specifications and executes automated or browser-based QA. | Operates during the `TESTER` phase of `/gal pipeline`, driving independent verification. |
| `golem-auditor` | Performs targeted security and performance audits on single tasks. | Operates during the `AUDITOR` phase of `/gal pipeline`. Orchestrator-driven only. |
| `golem-debugger` | Troubleshoots defects using scientific methodology, reproduction freezes, and root-cause isolation. | Run `/gal debugger` to isolate regressions before attempting fixes. |
| `golem-steward` | Maintains documentation structure, monitors code-to-doc drift, extracts durable knowledge, and syncs diagrams. | Run `/gal steward`, or rely on automatic triggers at plan creation, refinement, and finalization. |
| `golem-releaser` | Researches deployment tooling and drafts release plans. Operates advisory-only without executing deployments. | Run `/gal releaser` (isolated) or `/gal discuss releaser` (in-context) before `/planning release-<slug>`. |

**The Quality Triangle**: Verification responsibilities are split across three distinct owners. The **ORCHESTRATOR** enforces task correctness gates, terminal goal verification, and plan lifecycle transitions. The **AUDITOR** performs deep security and performance checks on individual task diffs. The **STEWARD** ensures documentation structural integrity and persistent knowledge retention.

### Golem Invocation Matrix

| Role | Directly Invocable | Invocation Syntax |
| --- | --- | --- |
| **architect** | Yes | `/gal architect` (isolated) or `/gal discuss architect` (in-context). |
| **analyst** | Yes | `/gal analyst` (isolated) or `/gal discuss analyst` (in-context). |
| **designer** | Yes | `/gal designer` (isolated) or `/gal discuss designer` (in-context). |
| **releaser** | Yes | `/gal releaser` (isolated) or `/gal discuss releaser` (in-context). Advisory planning persona only. |
| **debugger** | Yes | `/gal debugger`. |
| **steward** | Yes | `/gal steward`. |
| **implementer** | **No** (Orchestrator-driven) | Invoked exclusively by `/gal pipeline` during the `CODER` phase. |
| **tester** | **No** (Orchestrator-driven) | Invoked exclusively by `/gal pipeline` during the `TESTER` phase. |
| **auditor** | **No** (Orchestrator-driven) | Invoked exclusively by `/gal pipeline` during the `AUDITOR` phase. The legacy `--finalize-branch-audit` flag is deprecated and returns `COMMAND: error`. Finalize reviews run in-runtime, recording `Review Independence: DEGRADED_SAME_RUNTIME`. |
| **researcher** | **No** (Orchestrator-driven) | Invoked exclusively via `/gal research` or `/gal deep-research`. |

Attempting to run `/gal <role>` for orchestrator-driven roles outside their authorized pipeline context returns `COMMAND: error`.

### Dual Consultation Modes (`/gal discuss <role>`)

The review roles (`architect`, `analyst`, `designer`, and `releaser`) support two interaction modes:

| Mode | Command | Execution Pattern | Response Tag |
| --- | --- | --- | --- |
| **Isolated** (Default) | `/gal <role>` | Launches a dedicated subagent to complete the task independently, returning only a structured summary and verdict. | `[<role> · isolated]` |
| **In-context** | `/gal discuss <role>` | Loads role instructions directly into the active session, allowing conversational follow-ups across multiple turns. | `[<role> · in-context]` |

**Hot-join workflow**: When switching from isolated mode to in-context discussion, the assistant preserves the previous isolated verdict in conversation history, continuing discussion without re-executing initial evaluations.

Syntax varies by runtime: Codex translates `/gal discuss <role>` to `$discuss-<role>`, while Claude Code handles `/gal discuss <role>` as a native slash command.

All review roles utilize the [`adversarial-review`](../plugins/gal-core/skills/adversarial-review/SKILL.md) method: establishing a steel-man interpretation first, applying default skepticism, enforcing evidentiary standards, and delivering explicit `APPROVE`, `REVISE`, or `REJECT` verdicts.

### Steward Responsibilities Across Phases

The steward agent performs distinct functions across planning and finalization phases:

- **Planning Phase (`/deep-planning`)**: Enforces structural validity across plan files. Validates file naming, section completeness, language consistency, and diagram alignment. The steward is forbidden from writing to `docs/` during this phase, preventing unbuilt or speculative designs from polluting published documentation.
- **Finalize Phase (`/gal finalize`)**: Extracts durable knowledge into permanent documentation. The steward synchronizes validated knowledge from completed plans into `README.md` and `docs/`, then updates `.dev/project.md`. This documentation commit must land before plan files can be deleted.

Knowledge extraction occurs only after features are fully implemented and verified. The planning-stage steward focuses exclusively on plan formatting, maintaining strict separation between work-in-progress drafts and canonical documentation.

### Release Workflow (`release-<slug>`)

Release workflows are structured as dedicated plans (`release-<slug>`) running independently of regular feature pipelines:

1. Run `/gal releaser` to evaluate deployment readiness. The agent inspects available skills, APIs, and CLI tooling using read-only operations to design an appropriate release workflow. The command writes no files, makes no commits, and performs no deployments. Unsupported capabilities are marked `not-available`.
2. Run `/planning release-<slug>` to translate release recommendations into an actionable task plan with a `## Tasks` section.
3. Run `/gal pipeline` to execute the approved release plan.
4. Run `/gal finalize` to record release completion, following standard plan closure procedures.

This process supports both internal CLI packaging and downstream application deployments (such as container builds, npm packages, web deployments, or cloud services). The releaser provides architecture and automation designs only. It does not execute live deployment commands, canary rollouts, or production monitoring.

## Executing Workflows

For visual workflow diagrams and lifecycle overviews, see the [Feature Lifecycle](../README.md#the-feature-lifecycle). The sections below describe manual checkpoints, human interventions, and recovery procedures.

### Checking Status and Suggested Next Actions

The `/gal status` command inspects `.dev/state.md` and the active execution prompt, outputting a complete state projection: active plans, current phase, test results, blockers, continuity pointers, and agent readiness.

The `/gal whats-next` command inspects the same state files to answer a single question: what command should be run next. It locates the nearest parent directory containing `.dev/state.md` and outputs the recommended command with minimal context. Use `/gal whats-next` for quick orientation, and use `/gal status` when you require full diagnostic detail.

### Human Decisions in Planning

The `/planning` command initializes a source plan at `.dev/plans/<type>-<slug>.md`. The planning stage is collaborative: you can adjust requirements, consolidate plans, or consult specialist golems at any time.

**Resolving open questions**: Running `/deep-planning` requires all items in `## Open Questions` to be resolved before `/refining-plan` can proceed. Questions are categorized into three classes:

- **Class H**: Must be decided directly by a human operator.
- **Class A**: May be resolved autonomously by the `architect` role with documented rationale.
- **Class F**: Non-substantive questions or minor points.
Unlabeled questions default to Class H.

**Recording approval**: Once `/refining-plan` completes, it records the decision in the plan's `## Approval` section. The `/plan-to-prompt` command requires this section to contain `- Human approval: [approved]`. It refuses to generate execution prompts without explicit recorded approval.

**Targeting specific plans**: When multiple plans are active, specify the target plan path explicitly in planning commands. GAL never selects an active plan automatically.

### Importing External Plans

Use `/gal import-plan <source>` to import plans prepared in external tools, bypassing the standard `/planning` → `/deep-planning` → `/refining-plan` → `/plan-to-prompt` pipeline. The `<source>` argument accepts local file paths or raw pasted text. Supported formats include Spec-Kit `tasks.md`, BMAD stories, GSD plans, Claude or Codex planning exports, and freeform text.

Importing requires two sequential commands:

1. Run `/gal import-plan <source>`. GAL converts the input into `.dev/plans/<slug>.prompt.md`, creates a corresponding placeholder plan, and executes the preflight gate `gal prompt-check --assemble-dry-run --receipt <path>`. This gate validates that tasks have descriptive titles, specifications assemble cleanly, and file allowlists are non-empty. If checks fail, import aborts immediately.
2. Run `/gal pipeline`. When multiple plans exist, specify the newly imported prompt path explicitly.

Imported prompts include an `imported-from:` entry in their `## Status` section detailing source tool metadata or marking the plan as `pasted`. This annotation provides provenance tracking only. External documents are processed strictly as data and cannot dictate control-plane commands.

Imported plans skip formal architectural review gates. To compensate, correctness relies on strict task atomicity, granular task decomposition, and dry-run assembly validation.

### Pipeline Execution: Start, Pause, Resume

Run `/gal pipeline` to begin task execution. If a single plan is active, GAL automatically targets its execution prompt. If multiple plans are active, specify the target prompt path (`.dev/plans/<slug>.prompt.md`) explicitly.

The pipeline processes tasks sequentially through a fixed lifecycle: implementation, verification testing, audit review, commit creation, and handback inspection. Following each task, the handback inspector determines next actions: advance to the next task, initiate a retry, pause for human input, or transition to finalization. Operator intervention is required only when flagged by inspectors or when runtime limits are reached.

Retry rounds require updated authorization and concrete code modifications. Following a test or audit failure, the pipeline records an unresolved retry handoff note and re-dispatches the implementation worker with the `--fix` flag. If the subsequent attempt repeats identical goals, handoff notes, file allowlists, and contracts, the retry is rejected before execution starts. If the executor exits cleanly without modifying allowlisted files, the round fails with `fix-round-no-change`, since updating prompt logs or receipts alone does not constitute substantive work.

The pipeline halts if an unresolved blocker requires human decisions, if a task reaches the three-failure retry limit, if repository branches diverge, if scope boundaries are challenged, if goal-gap checks fail, or if configured working-hours Hard Stop thresholds are reached. Runtime boundaries output interruption notes and resume markers without generating unearned approvals.

To recover from interruptions, resolve the root cause and rerun the same `/gal pipeline` command. If an earlier attempt was rejected by replay detection, update the handoff notes with distinct remediation details. Completed tasks are preserved and skipped automatically on subsequent runs.

### Task Execution Cycle and Roles

Within each task, the pipeline follows a strict execution loop: task validation, cursor update, code implementation, reconciliation, commit creation, testing, auditing, and convergence. The ORCHESTRATOR oversees task validation, creates standard Git commits, updates tracking cursors, and determines finalization readiness.

- **Task Validation**: The ORCHESTRATOR checks task criteria against `plugins/gal-core/conventions/task-quality.md`. If required criteria are missing, the pipeline halts before dispatch, requiring plan refinement via `/refining-plan`.
- **Implementation**: The `CODER` receives the task specification, agent contract, and repository context. It modifies only allowlisted files and returns an execution log without creating Git commits.
- **Reconciliation and Commit**: The ORCHESTRATOR verifies that modified files match allowlists, runs `gal boundary-check`, and creates a `Task Final Commit` using standard `git commit`.
- **Testing**: The `TESTER` evaluates the `Task Final Commit`, executes test cases matching `TP-NN` definitions, and records results under `## Test Results`. The tester creates no commits.
- **Auditing**: The `AUDITOR` inspects changes across the `Task Base Commit..Task Final Commit` range, appending evidence-backed findings under `## Review Results`.
- **Convergence**: The ORCHESTRATOR evaluates outputs and receipts. If discrepancies appear, it re-dispatches the responsible role. If tests or audits fail, it re-dispatches implementation using `--fix`, recording resulting changes under an updated `Task Final Commit`.

Roles operate independently. Dispatched workers never receive internal handoff notes, raw diffs, or receipts generated by other roles.

### Finalization: Artifact Persistence and Cleanup

The `/gal finalize` command coordinates the formal completion of approved plans. Execution is gated by `gal finalize-check`, which verifies that all tasks are complete, tested, and audited. The `gal finalize-check` gate is strictly read-only: it inspects repository files, state manifests, and plans without altering code or documentation, writing only the requested verification receipt.

**Persistent updates**:

- Prior to documentation synchronization, a top-down requirement audit evaluates changes across four layers: L1 facts, L2 files, L3 wiring, and L4 trust boundaries. Findings are recorded in a `### Finalize Review <date>` table under `## Review Results`.
- The `golem-steward` extracts verified knowledge into `README.md` and `docs/`.
- Merges to the mainline branch execute, and temporary worktrees are removed if applicable.

**Automated conflict resolution**: If Git merge conflicts occur exclusively within `{.dev/state.md}`, finalize invokes `gal state-merge`, which merges rows deterministically by plan key. If resolution succeeds, finalization continues. If `STATE_MERGE: unresolved` is reported, finalization halts with the repository preserved in its pre-merge state. Any other file conflict halts finalization immediately.

**Artifact cleanup**:

- Plan files in `.dev/plans/` are deleted only after documentation commits land and hygiene checks pass, ensuring knowledge is preserved before working drafts are removed.
- Once plans are deleted, finalize executes `gal pipeline-clean` to remove `.dev/pipeline/<plan-slug>/`.

**State recording**:

- A completion entry is written to `.dev/state.md` recording the date, plan slug, and final commit hash.
- Non-blocking review findings from `### Finalize Review <date>` are copied to the `## Follow-ups` section of `.dev/state.md`, retaining the five most recent items.
- The `gal-last-good` tag is updated to point to the finalize commit.

## Headless Dispatch

### Architectural Overview

GAL can delegate specific pipeline phases to secondary headless coding agent CLIs. Delegation is configured via `executorRouting` in `~/.gal/config/config.json`. For schema definitions, combination registries, and fallback logic, see [configuration.md](configuration.md#executor-routing-executorrouting). This section covers execution mechanics, diagnostics, and recovery.

### Role Roster

| Role | Group | Scope |
| --- | --- | --- |
| `CODER` | pipeline | Implements code changes specified by the active task. Should run on a different model than `TESTER`. |
| `TESTER` | pipeline | Authors and executes tests against public APIs and specifications. Should run on a different model than `CODER`. |
| `AUDITOR` | pipeline | Evaluates performance characteristics and security boundaries. Must run on a model equal to or higher in capability than `CODER`. |
| `ARCHITECT` | planning | Adversarial plan review assessing design trade-offs and latent architectural risk. |
| `ANALYST` | planning | Business-logic analysis verifying domain rules, ROI, and user workflows. |
| `DESIGNER` | planning | User experience, UI styling, and developer tooling review. |
| `RELEASER` | planning | Designs deployment and DevOps flows. Read-only advisory role. |
| `RESEARCHER` | research | Conducts isolated research as one of three parallel workers (`RESEARCHER#0`, `#1`, `#2`). |

The `pipeline` group uses headless CLI dispatch. The `planning` group relies on model routing within native subagents and does not execute headless dispatch. The `research` group runs parallel headless workers across up to two configured models.

### Adapter CLI Behaviors

- **opencode**: Dispatches with `--agent build` to provide write permissions, alongside `--auto` for automatic permission approval.
- **GitHub Copilot**: Adds `--no-custom-instructions` and `--disable-builtin-mcps` to stay within prompt token budgets. Copilot Free requires `model: auto` and runs exclusively in local execution mode.

### Security Warning: Permission Bypass

Headless dispatch invokes secondary CLIs with automated permission approval flags: `--dangerously-skip-permissions` for Claude Code and Antigravity, `--auto` for opencode, `--allow-all` for Copilot, and `-s workspace-write` for Codex. This grants broad read and write access across your local filesystem and shell environment, effectively bypassing interactive sandbox prompts.

Enable executor routing only on trusted development systems and within secure repositories. Never execute automated routing against untrusted source code or unverified task contracts. Although agent prompts instruct models not to run `git commit` or `git push`, this constraint is enforced via instructions rather than operating-system boundaries.

### Remote Dispatch (SSH Lane)

Remote execution is configured directly within `executorRouting` rather than via dedicated commands. The `/gal pipeline` workflow behaves identically whether workers run locally or across remote SSH sessions.

**Host prerequisites** (must be provisioned manually before dispatch):

- Target systems must support non-interactive SSH authentication (`BatchMode=yes`). Interactive password or passphrase prompts are treated as connection failures.
- Target coding CLIs (`claude`, `codex`, `agy`, `opencode`) must be installed and logged in on the remote machine. The `gal` binary is not required on remote hosts.
- Before dispatch, the remote checkout designated in `remoteWorkdir` must match the local Git `HEAD` commit and have a clean working tree.

**Execution flow**: Remote dispatches execute synchronously across an SSH session. After a modifying task completes successfully, GAL retrieves uncommitted remote changes and applies them to the local control repository. Only after local changes apply cleanly does GAL reset the remote repository (`git reset --hard && git clean -fd`). If applying local changes fails, the remote repository is left untouched for troubleshooting.

The `remoteWorkdir` setting must point to a **dedicated GAL checkout**. Never point this path to an interactive working checkout, as post-task cleanup permanently discards uncommitted files.

**Operational limitations**: Copilot does not support remote SSH execution due to CLI parameter limitations, and attempting remote Copilot dispatch returns an error. Windows remote hosts are unsupported because execution depends on POSIX shell environments. SSH connections do not support automatic reconnects or mid-task resume. If an SSH session drops, the task fails immediately. Remote execution runs single tasks sequentially: because the local control repository advances its `HEAD` commit after each task, remote checkouts must be synchronized before subsequent remote dispatches can run.

### Worker Isolation Boundaries

When `/gal pipeline` dispatches a task to a headless executor, the secondary agent functions as an execution worker rather than a workflow orchestrator. To prevent secondary agents from misinterpreting repository instructions (such as workflow obedience markers) as directives to run pipeline commands, GAL enforces role isolation:

1. **Task specification scoping**: Rendered task specifications embed a `## Dispatched Worker Boundary` block above `## Task Goal`. This section restricts the agent to its specific task assignment. It strictly forbids executing `gal` subcommands (including `gal pipeline-preflight`, `gal pipeline-handback-check`, `gal boundary-check`, and `gal pipeline`) because orchestrator gates are managed centrally. It also forbids loading workflow skills. Dispatched workers must follow only their designated task goals, file allowlists, and inline `## Agent Contract`.
2. **Instruction suppression**: Secondary CLIs often auto-load repository instruction files (`AGENTS.md` or `CLAUDE.md`) into system prompts. Where supported, GAL passes flags to suppress repository-level instructions, ensuring only the self-contained task specification guides the model.

| Executor | Repository Instructions | Suppression Mechanism | Verified Version | Behavior |
| --- | --- | --- | --- | --- |
| **codex** | Exposed (`AGENTS.md`) | `-c project_doc_max_bytes=0` | codex-cli 0.149.1 | Passes `-c project_doc_max_bytes=0` to suppress local `AGENTS.md`. User-level `~/.codex/AGENTS.md` remains active. |
| **copilot** | Exposed | `--no-custom-instructions`, `--disable-builtin-mcps` | Copilot CLI | Disables repository instructions and built-in MCPs to maintain self-contained prompt context. |
| **claude** | Exposed (`CLAUDE.md`) | `--setting-sources user` | Claude Code 2.1.251 | Passes `--setting-sources user` to ignore local `CLAUDE.md` and repository configs while preserving user settings. |
| **opencode** | Inconclusive | None | opencode 1.18.25 | Discovery timed out during testing, so no suppression flags were applied. Treat isolation status as unverified. |

## Dispatch Logging and Diagnostics

Each dispatch generates two persistent operational records per task.

### Layer 1: GAL Executor Logs

Executor logs provide a standardized audit trail across all supported tools. Logs are stored under `.dev/pipeline/<plan-slug>/<task>/` (or `.dev/pipeline/<yyyymmdd>/test-direct/` for manual dispatches) using the naming pattern `<timestamp>-<attempt>-<task>-<phase>-<executor>.log`. The file header records terminal status, exit codes, resolved model names, Git commit hashes, and provider session identifiers. The `---STDOUT---` section captures full provider event streams: messages, shell executions, file edits, and token usage. Always consult these logs when evaluating agent actions.

Log headers classify runs into standard terminal statuses:

| Status | Description |
| --- | --- |
| `completed` | Process exited with code 0, receipt was confirmed, and worktree modifications matched expectations. |
| `no-receipt` | Process exited with code 0, but the expected receipt file was missing, empty, or unreadable. |
| `workdir-escape` | Legacy status retained for historical log analysis, not emitted by current dispatchers. |
| `no-writeback` | Process exited with code 0 and returned a receipt, but the working directory contained zero modifications. |
| `timeout` | Process was terminated for exceeding the configured timeout threshold. |
| `timeout-no-output` | Process timed out before emitting output. |
| `timeout-midrun` | Process timed out after emitting partial output. |
| `disconnected-partial` | Process exited with a non-zero exit code. |
| `unavailable` | The requested executor CLI was not found on system `PATH`. |

This table serves as the authoritative terminal status registry. Note that `started` represents an in-flight execution marker rather than a terminal status.

The `no-writeback` status adheres to specific operational boundaries:

- **Phase scope**: Applies strictly to the `implement` phase. It does not apply to `audit` or `test` phases, which write results to receipts in `.dev/pipeline/` rather than modifying tracked repository files.
- **Content hashing**: If files retain identical `git status --porcelain` markers before and after execution (such as pre-modified files), change detection compares SHA-256 content hashes to avoid misclassifying real edits as `no-writeback`.
- **Ignored paths**: If all modifications fall within gitignored paths, execution is reported as `no-writeback` because working tree monitors track version-controlled files only.
- **Contract coupling**: Status classification depends on the phase delivery contract: `implement` delivers changes via tracked files, whereas `audit` and `test` deliver changes via receipts.

### Tracking `contract` and `contract_source` Metadata

Prompt-driven dispatches include `contract=<path> contract_source=workdir|ancestor|exe-side|embedded` within `Dispatch:` log markers and header sections. These fields identify the exact agent contract executed by the worker:

- `workdir` / `ancestor`: Contract loaded from a local GAL checkout.
- `exe-side`: Contract loaded from files adjacent to the active `gal` binary.
- `embedded`: Contract extracted from embedded defaults at `~/.gal/embedded-src`.

Raw `gal dispatch` executions and direct task-spec dispatches omit these metadata tags because they do not evaluate provenance hierarchies.

### Reasoning Effort Metadata (`effort`)

Dispatches with provenance tracking append an `effort=<value|(default)>` parameter to log headers. This value is resolved during initial route selection and remains constant throughout the task. Unrouted dispatches omit this parameter to maintain backwards-compatible log formats. When routing generates an `OFFLOAD` directive, `gal dispatch-script` renders a standard reporting line (`Dispatched: <phase> <T-NN> - <ROLE> as <executor>, model <model>, effort <effort>`) for orchestrator output.

### Layer 2: Native Session Resume

Log headers record the provider's `session_id`, allowing operators to resume sessions in native CLI interfaces:

| Executor | Native Resume Command | Default Visibility |
| --- | --- | --- |
| **claude** | `claude --resume <session_id>` | Visible in session history. |
| **codex** | `codex resume <uuid>` | Hidden by default. Use `--include-non-interactive` to display in menus, or `--all` to ignore directory filters. |
| **opencode** | `opencode run -s <session_id>` (or `opencode export <session_id>`) | Visible in session history (`opencode session list`). |
| **copilot** | `copilot --resume=<session_id>` | Stored in `~/.copilot/session-store.db`. Resumable by ID, with no listing command. |
| **agy** | `agy --conversation <uuid>` | Stored in `~/.gemini/antigravity-cli/brain/<uuid>/`. Browsable by ID only. |

Consult Layer 1 executor logs for automated verification evidence. Use Layer 2 commands when manual intervention in native runtime environments is needed.

### Executor Smoke Tests (`gal doctor --executor-smoke`)

The `gal doctor --executor-smoke` command validates headless dispatch workflows across `codex`, `claude`, `copilot`, `agy`, and `opencode`. It uses isolated test routing configurations to dispatch a minimal verification task that writes a single receipt line. Results are recorded in `.dev/pipeline/<yyyymmdd>/test-executor-smoke-<HHMMSS>Z/local/` containing JSON summaries, readable status tables, and raw logs.

| Status Code | Description |
| --- | --- |
| `PASS` | Dispatch succeeded, and the receipt was verified. |
| `NOT_INSTALLED` | The CLI binary is not present on `PATH`. |
| `NOT_AUTHENTICATED` | The CLI is not logged in. Run the tool's login command. |
| `AUTH_UNKNOWN` | Authentication status could not be pre-verified. Executes a test task to determine status. |
| `UNSUPPORTED` | The specified executor name is not one of the five supported tools. |
| `CONFIG_ERROR` | Configuration validation failed before dispatch (e.g., unsafe `--report-dir`). |
| `CALL_FAILED` | Executor exited with a non-zero exit code. Check the executor log. |
| `NO_RECEIPT` | Process exited with code 0 but generated no receipt file. |
| `TIMEOUT` | Execution exceeded the timeout limit. Adjust `--timeout` if models respond slowly. |

Remote smoke tests use `--transport ssh --ssh-target <target> --remote-workdir <dedicated checkout>`. Host accessibility and login checks execute directly **on the remote machine**. Reports are written to `.dev/pipeline/<yyyymmdd>/test-executor-smoke-<HHMMSS>Z/ssh/`. The `--remote-workdir` directory must point to a dedicated checkout synchronized with the control repository's Git `HEAD`.

| Remote Status Code | Description | Recommended Action |
| --- | --- | --- |
| `SSH_UNREACHABLE` | Non-interactive SSH connection could not be established. | Check host network reachability and SSH key authentication. |
| `REMOTE_GUARD_FAILED` | Remote checkout is missing, dirty, or out of sync with control `HEAD`. | Resync the dedicated remote checkout to match local `HEAD` and clean untracked files. |
| `REMOTE_FETCH_FAILED` | Remote command completed, but receipt retrieval failed. | Inspect executor logs and remote receipt paths. |

Remote Copilot returns `UNSUPPORTED` due to remote transport limitations. Passing smoke tests requires both clean process completion **and** retrieval of valid, non-empty receipt files.

When running without `--strict`, the command exits with code 0. Diagnostic tables and JSON logs serve as the definitive source of test results.

## Workflow Recovery

### Resolving Finalize Gate Failures

Run `gal finalize-check` and inspect all output lines. In complete verification runs, checks execute in fixed order: `authoritative-command`, `naming-gate`, `sync-idempotency`, `finalize-mode`, `project-source-doc-existence`, `state-bound`, `contract-roster-parity`, `doc-link-resolution`, and `working-tree-clean`. All checks must report `pass`. Repositories without a root `plugins/gal-core/` directory evaluate seven checks, omitting `contract-roster-parity` and `doc-link-resolution`.

The `sync-idempotency` check verifies that candidate adapter renderings remain deterministic across multiple passes and does not write changes to disk. Differences between projected files and disk state are reported as drift and do not fail this gate.

The `authoritative-command` check reports execution exit codes and process errors. The `working-tree-clean` check reports uncommitted file counts and status output. Other checks report concise summary statuses.

Hygiene verification evaluates seven checks: `project-source-doc-existence`, `state-bound`, `durable-layer-commit`, `finalize-review-shape`, `contract-roster-parity`, `doc-link-resolution`, and `working-tree-clean`. This confirms clean repository state and valid `### Finalize Review <date>` structures before plan files are removed. Skipped checks (`NotRun`) or missing evidence are treated as failures.

Troubleshoot gate failures by category:

- Authoritative command errors, environment faults, or uncommitted files require independent remediation. Do not attempt ad-hoc fixes during finalization.
- Empty or invalid rows in `### Finalize Review <date>` fail `finalize-review-shape`. Re-run reviews to generate valid table entries before rechecking hygiene.
- If uncompleted tasks remain while the plan status is not `DONE`, return to `/gal pipeline` to complete the remaining tasks.
- If a plan is marked `DONE` while unverified or contradictory state remains, treat this as state corruption. Stop immediately and document findings in handoff notes.

Failed finalize gates never permit automatic overrides or manual tampering with receipts.

Terminal recovery runs only after committed states are confirmed clean:

1. Run `gal pipeline-preflight --terminal-reverify <prompt-path>`.
2. Once verified, the ORCHESTRATOR executes goal-backward verification in the current process.
3. Run `gal pipeline-handback-check <prompt-path>`.
4. Run `gal finalize-check <prompt-path>`.

### Clearing Stalled Installation Leases

Modifications to `crates/` trigger rebuild and reinstallation flows wrapped in an advisory lease located at `~/.gal/.locks/gal-install/`. This lease is machine-global and shared across all local checkouts. Concurrent runs check this lock every 5 seconds, timing out after 900 seconds while reporting the lock path without removing it automatically.

If a pipeline or build crashes, the lock file can remain in place, blocking subsequent builds. To resolve this:

1. Verify that no other GAL pipeline or build processes are running on the system.
2. Manually delete the directory `~/.gal/.locks/gal-install/`.

Automatic deletion is intentionally not supported. Cold release builds (`cargo build --release`) on busy machines can exceed 900 seconds. Automated deletion would prematurely release locks during legitimate compilation.

Before trusting gate receipts, GAL recomputes the SHA-256 hash of the `gal` executable and verifies it against the hash recorded during bootstrap. If hashes diverge, the executable changed mid-run, rendering receipts invalid. Rerun affected gate checks using the verified binary.

### Clearing Receipt Leases

Receipt leases prevent concurrent dispatches from overwriting deterministic receipt files. Messages such as `reason=receipt-preparation-failed` or `reason=remote-receipt-freshness-failed` indicate active or orphaned lock files. They do not grant permission to delete receipt directories indiscriminately.

Inspect stderr for the specific lock path: `.dev/pipeline/.locks/<hash>.lock` (or `.dev/pipeline/.locks/<hash>.lockdir` for remote runs). Before clearing locks, ensure that associated local or SSH processes are no longer executing. Once verified, delete only the specific lock file or the remote `owner` file within its lock directory. **Never delete the entire `.locks/` folder.**

Receipt failure codes:

| Failure Code | Description |
| --- | --- |
| `remote-receipt-fetch-failed` | Target receipt is missing, empty, a symlink, or unreadable. |
| `remote-receipt-fetch-timeout` | Receipt download exceeded the network timeout limit. |
| `remote-receipt-fetch-read-failed` | Pipe read failure resulted in incomplete stdout capture. |
| `remote-receipt-fetch-too-large` | Receipt output exceeded the 1 MiB size limit. |
| `receipt-lease-cleanup-failed` | Local lock file could not be removed during normal cleanup paths. |
| `remote-receipt-lease-cleanup-failed` | Remote owner lock could not be cleared, so locks fail closed for safety. |

Codes ending in `-unconfirmed` indicate that local helper SSH processes exited without confirmation. Exit code 73 indicates a normal process return code rather than a lease failure unless accompanied by specific freshness sentinels.

### Handling Semantic Write-Back Failures

For prompt-driven dispatches (`audit` or `test`), an exit code of 0 indicates that a receipt was delivered to `<task>-<phase>.receipt.md`. It does not mean the task is complete or that prompt files were updated. Updating prompt content is managed exclusively by the `gal` control node, which validates payloads against a fail-closed semantic write-back gate.

**Failure symptoms**: If validation fails, `gal` prints `phase-writeback semantic failure for task <T-NN>` to stderr, logs error details to the pipeline log, and exits with a non-zero code. The on-disk execution prompt remains unmodified, preserving prompt immutability.

Operational scope:

- Receipts from local and SSH dispatch paths pass through the same semantic gate, ensuring consistent prompt updates.
- Raw specifications (`PipelineInput::RawSpec`) and direct dispatches (`gal dispatch`) bypass write-back gates because they operate without authoritative prompt files.

Troubleshooting write-back errors:

- **Outdated agent contracts**: If an executor outputs malformed Markdown (such as missing verdict markers or stray headings) or attempts to edit prompt files directly, run `gal refresh` to resynchronize projected contracts.
- **Malformed receipt structures**: Check receipt files in `.dev/pipeline/<plan-slug>/...` or consult the pipeline log for syntax issues, such as missing `### [T-NN] YYYY-MM-DD` headers or mismatched task identifiers.
- **Rerunning tasks**: After addressing contract or formatting errors with `gal refresh`, rerun `/gal pipeline`. Never paste Markdown snippets into execution prompts manually. Automated placement ensures content is inserted under the proper `## Test Results` or `## Review Results` headings.

## Internal Subcommands and Automation Gates

The subcommands below are invoked automatically by workflows and do not form part of the public `/gal` command surface. They appear primarily in diagnostic logs and gate outputs:

| Subcommand | Workflow Phase | Primary Function |
| --- | --- | --- |
| `gal planning-check` | Planning | Validates structural requirements for initial plans. |
| `gal refining-check` | Refining | Validates structural consistency in source plans. |
| `gal prompt-check` | Prompt Generation | Validates prompt schemas. Use `--assemble-dry-run` to test task assembly. |
| `gal planning-stamp` | Planning | Records planning authority stamps. Use `--equivalence <prompt>` to verify semantic parity between localized plans and English prompts. |
| `gal pipeline-preflight` | Pipeline Entry | Entry verification gate. Must return `pass` before code modifications begin. Use `--terminal-reverify` during recovery. |
| `gal boundary-check` | Pre-commit | Compares modified files against allowlists in `## Affected Files`. |
| `gal pipeline-converge-check` | Task Convergence | Validates that task receipts and returns match. |
| `gal pipeline-handback-check` | Pipeline Exit | Validates task completion evidence before finalization. |
| `gal pipeline-log append` | Global | Writes structured entries to pipeline event logs. |
| `gal pipeline-clean` | Finalize | Cleans up temporary artifacts in `.dev/pipeline/<plan-slug>/`. |
| `gal state-merge` | Finalize | Deterministically merges `.dev/state.md` tables by plan key. |
| `gal finalize-check` | Finalize | Validates zero-trust preconditions during finalization. |
| `gal dispatch-script` | Dispatch | Generates control-plane dispatch blocks, including `OFFLOAD` directives. |
| `gal dispatch` | Dispatch | Raw execution entry point bypassing provenance tracking and write-back gates. |

For maintainer commands (`gal restore`, `gal release`, `gal release-notes`, `gal marketplace-snapshot`), see [CONTRIBUTING.md](../CONTRIBUTING.md#maintainer-subcommands).

## Additional Lifecycle Operations

### Agent Contract Resolution

Before dispatching tasks, `gal` locates the authoritative contract (`agents/golem-{implementer|tester|auditor}.agent.md`), reads its content, and embeds it directly within the task specification. Because task specifications are entirely self-contained, remote executors never need access to control node paths.

Contract resolution searches four tiers in order of precedence:

1. `workdir`: The normalized `--workdir` path, or its immediate `plugins/gal-core` subdirectory.
2. `ancestor`: The nearest parent directory recognized as a GAL source root.
3. `exe-side`: Directory adjacent to the running `gal` binary.
4. `embedded`: Embedded source files extracted to `~/.gal/embedded-src`.

The `workdir` location takes highest priority. This ensures local checkouts containing `plugins/gal-core/` remain authoritative even if an older binary runs from `PATH`. When managing source checkouts, inspect the `contract_source` entry in `Dispatch:` log headers to detect version drift.

Handling resolution failures:

- **Corrupted source root**: If the highest-priority root contains unreadable or non-UTF-8 contracts, dispatch halts immediately with an error naming the affected path. Resolution never falls back to lower tiers automatically. Resolve file errors in the designated root.
- **No valid roots found**: If all tiers fail to yield a contract, dispatch exits with code 1. Resolve this by reinstalling `gal` via your package manager or running commands from a valid source checkout.

Raw `gal dispatch` executions and raw task specifications bypass contract resolution, omitting `contract=` and `contract_source=` fields from logs.

### Session Wrap-Up vs. Finalization

The `/gal wrap-up` command pauses active sessions without completing plans. It consolidates handoff notes into the execution prompt, updates session tracking in `.dev/state.md`, and commits changes to allow resuming from any supported runtime. It does not close the plan. Use `/gal finalize` to close completed plans, and use `/gal wrap-up` when pausing work in progress.

## Git Workflow Helpers

### Commit Message Generation (`gal commit-msg`)

The `gal commit-msg` command powers the `git-commits` skill and the `git-commit-msg` command. It determines commit types and scopes deterministically from staged file paths and Git state, ignoring diff bodies and commit messages:

- `gal commit-msg --context`: Outputs formatted context for staged changes (file lists, baseline subject lines, and diff hunks) for model reference.
- `gal commit-msg --print`: Outputs the deterministic type-and-scope subject line directly.
- `gal commit-msg <file>`: Functions as a Git `commit-msg` hook, populating empty commit templates from staged changes without overwriting existing drafts.

The `git-commit-msg` command generates message proposals without creating commits. In contrast, the `git-commits` skill creates commits directly when explicit user intent is confirmed.

### Git Filters (`gal clean` and `gal smudge`)

The optional `gal-config` filter removes machine-local settings from tracked repository files:

```bash
git config filter.gal-config.clean "gal clean"
git config filter.gal-config.smudge "gal smudge"
```

Git invokes these filters during checkout and commit operations. Ensure `gal` is available on system `PATH` to avoid commit failures caused by filter errors.

## Visual Documentation Helpers

### Decision Tree Charts (`text-flowcharts`)

The [`text-flowcharts`](../plugins/gal-core/skills/text-flowcharts/SKILL.md) skill generates monospaced ASCII and Unicode decision trees for documenting branching logic, pipelines, and state transitions. In Claude Code, invoke it via `/text-flowcharts`. It also activates automatically when describing conditional control flows or when flowcharts are requested.

Charts trace individual records from entry points through sequential conditions to terminal outcomes. The visual notation uses standard characters:

| Flow Element | Notation Characters |
| --- | --- |
| Lines and corners | `│ ─ ┌ ┐ └ ┘` |
| Junctions (branch, merge, cross) | `├ ┤ ┬ ┴ ┼` |
| Directional arrows | `▼ ▲ ▶ ◀` |
| Success outcome | `√` |
| Intentional skip | `>>\|` |
| Rejected or terminated path | `×` |

These characters render cleanly across standard monospaced fonts without requiring special terminal fonts. They preserve alignment in GitHub pull requests, review comments, and terminal emulators. Output avoids emoji to maintain readability on legacy terminals. Use decision tree charts when workflows contain conditional branches. Simple linear procedures are better expressed as numbered lists.
