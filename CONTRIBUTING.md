---
type: Policy
title: Contributing to GAL
description: Contribution guidelines for GAL, covering branching conventions, commit standards, build and test gates, and review checklists.
tags:
  - contributing
  - commits
  - gates
  - review
status: stable
---

# Contributing to GAL

GAL consists of a Rust workspace and a collection of document-driven workflow contracts. Before making code changes, read [architecture.md](docs/architecture.md) to understand system structure and layer boundaries. Verify all changes against the verification gates described below. The governing workflow contract is [`coding.md`](plugins/gal-core/workflows/coding.md).

## Branches and Commits

- Create a feature branch from `main` and submit a pull request. Repository convention permits solo planning documents to be committed directly to `main`.
- Use scoped Conventional Commits, such as `feat(gal-engine): ...`, `refactor(projection): ...`, or `docs(...): ...`.
- Keep each commit focused on a single, independently revertible logical change.
- The pre-commit hook enforces the naming gate, `.dev/` commit policies, and personal-path leak checks. Ensure the `gal` executable is available on your `PATH` when triggering pre-commit hooks.

## Build and Test

Run the full verification suite locally:

```text
cargo build --workspace
cargo test --workspace
cargo clippy --workspace
cargo fmt --check
gal naming-gate
gal render-adapters
```

Every check must pass cleanly. Running `gal render-adapters` consecutively must produce an empty diff.

Unit tests belong in a `mod tests` block at the bottom of the corresponding source file. Integration tests belong in `crates/<crate>/tests/`. Test files under `crates/gal-engine/tests/*.rs` must not contain the words `install`, `setup`, `update`, or `patch` in their filenames. On Windows, executables matching those keywords trigger UAC elevation heuristics and raise spawn error 740 unless an `asInvoker` manifest is present.

If you edit files in `plugins/gal-core/`, run `gal refresh --source ./plugins/gal-core`. If you edit code in `crates/`, recompile the project, place the new binary on your `PATH`, and run the refresh command again. For details on projection lifecycles, see [projection.md](docs/projection.md).

## Contributor Gates

Non-trivial changes must advance through the standard lifecycle sequence: planning, deep planning (when required), plan refinement, human review and approval, prompt generation, implementation, testing, auditing, and finalization. Any work touching a Protected Path requires a reviewed and approved plan beforehand. The authoritative Protected Paths list is maintained in `.dev/project.md`.

Before submitting changes for review, verify that:

- Changes adhere to defined crate boundaries and dependency rules.
- The public CLI surface remains identical across all supported runtimes.
- Generated adapters remain perfectly synchronized with source templates in `plugins/gal-core/`.
- All newly added or modified tests were executed and passed. Do not mistake skipped tests (`NotRun`) for passes.
- Documentation accurately reflects current behavior without duplicating rules defined in authoritative files.
- Identifiers and terms strictly follow canonical definitions in [`docs/glossary.md`](docs/glossary.md).

## Conventions and Workflow Contracts

Consult the contract file relevant to your task before writing code. These documents serve as the authoritative standard for system behavior.

| Contract | When to read it |
| --- | --- |
| [`workflows/coding.md`](plugins/gal-core/workflows/coding.md) | Before starting implementation work. This is the mandatory coding workflow. |
| [`workflows/doc-sync.md`](plugins/gal-core/workflows/doc-sync.md) | When code changes require documentation impact analysis. |
| [`workflows/research.md`](plugins/gal-core/workflows/research.md) | When executing `/gal research` or `/gal deep-research`. |
| [`conventions/rust.md`](plugins/gal-core/conventions/rust.md) | Before making changes to Rust crates or Cargo configurations. |
| [`conventions/naming.md`](plugins/gal-core/conventions/naming.md) | When introducing or renaming crates, modules, CLI commands, configuration keys, or agent roles. |
| [`conventions/task-atomicity.md`](plugins/gal-core/conventions/task-atomicity.md) | When decomposing plans into atomic `T-NN` tasks. |
| [`conventions/task-quality.md`](plugins/gal-core/conventions/task-quality.md) | When drafting task requirements. The pipeline task linter reads this file directly. |
| [`conventions/open-questions.md`](plugins/gal-core/conventions/open-questions.md) | When defining or closing open questions (`## Open Questions`) across classes H, A, and F. |
| [`conventions/handoff-notes.md`](plugins/gal-core/conventions/handoff-notes.md) | When documenting task handoffs or handling retry authorizations. |
| [`conventions/token-budget.md`](plugins/gal-core/conventions/token-budget.md) | Complete token budget policy. A brief summary is included below. |
| [`conventions/minimalism.md`](plugins/gal-core/conventions/minimalism.md) | When evaluating whether a new abstraction or configuration option is necessary. |
| [`conventions/core-vs-personal.md`](plugins/gal-core/conventions/core-vs-personal.md) | When deciding between core contracts and the personal overlay layer. |
| [`conventions/self-bootstrap.md`](plugins/gal-core/conventions/self-bootstrap.md) | When managing rebuild and reinstallation workflows triggered by `crates/` modifications. |
| [`conventions/working-hours.md`](plugins/gal-core/conventions/working-hours.md) | When checking whether Wrap-up Time or Hard Stop rules apply. |
| [`conventions/optional-capabilities.md`](plugins/gal-core/conventions/optional-capabilities.md) | Before adding optional external integrations. Defines the five-state preflight check and graceful degradation rules. |

## Token Discipline

The following practices apply to all human contributors and automated agents working in this repository. The authoritative policy is defined in [`conventions/token-budget.md`](plugins/gal-core/conventions/token-budget.md):

- **Avoid reading generated files**: Do not inspect generated adapters (`AGENTS.md`, `CLAUDE.md`) or build artifacts (`bin/`, `obj/`) unless specifically auditing adapter generation. These files are large, change frequently, and carry no information beyond their source templates.
- **Perform targeted file reads**: Inspect only files specifically named in the active task or direct dependencies of those files. Avoid loading the entire codebase into model context during cold starts.
- **Report failures concisely**: For build commands, print only the first error with its file and line number, followed by a single summary line. For test runs, print only failing test names and assertion failures. For linters, print only violated rules and matching file locations. When full logs are needed, save them to disk and inspect relevant slices instead of loading entire logs into context.
- **Handle context pressure systematically**: If context limits approach exhaustion mid-task, take three actions. First, record the current task name, the last completed action, and key decisions under `### Handoff Notes` in the active plan. Second, update the plan row in `.dev/state.md` under `## Session Continuity`, setting `Stopped At` and `Next Step`. Third, do not create standalone `CONTEXT.md` files. State must remain consolidated within the active plan and `.dev/state.md`.

## Maintainer Subcommands

The following maintenance subcommands are reserved for repo maintainers and release workflows:

| Subcommand | Responsibility |
| --- | --- |
| `gal restore [--yes]` | Restores the source repository to the last `gal-last-good` tag, reverting both source and generated layers. |
| `gal release --version <v>` | Generates release artifacts, including `checksums.txt`, `artifact-manifest.json`, and package manager manifests. |
| `gal release-notes` | Generates release notes from merged changes. |
| `gal marketplace-snapshot --source <dir> --out <dir>` | Exports the public marketplace plugin tree. Emits only core assets without personal configurations or host binaries. |
| `gal naming-gate` | Enforces term registry rules during pre-commit checks. |
| `gal translation-freshness` | Evaluates translation freshness across `docs/i18n`. |

### Restoration Behavior (`gal restore`)

| Target | Restoration Behavior |
| --- | --- |
| **Source layer** | Executes `git reset --hard gal-last-good` and `git clean -fd`. Reverts the working directory to match the tagged commit exactly, removing untracked files. |
| **Derived layer** | Calls `gal refresh` to regenerate the canonical root and runtime projections from restored source files. |

The `gal restore` command never transfers machine-local settings between environments.

**The `gal-last-good` tag**: When `/gal finalize` runs, it creates a lightweight Git tag named `gal-last-good` on the final commit. The `gal restore` command relies exclusively on this tag as its baseline. If the tag is missing, the command fails closed with actionable guidance and does not guess or fall back to `HEAD`. Prior to the first execution of `/gal finalize` in a fresh repository, this tag does not exist, and `gal restore` safely refuses to run.

**Safety backup**: Before making any modifications, `gal restore` creates a backup branch named `gal-restore-backup-<ts>` at current `HEAD`. This check is mandatory. If the backup branch cannot be created, execution aborts immediately without modifying the working tree.

**Confirmation check**: If executed without `--yes`, `gal restore` prints uncommitted changes and the number of commits ahead of the tag, then exits with a usage reminder. This provides a safe dry-run preview.

## Troubleshooting

### Lock-File Write Order

During projection, `update_skills()` saves the lock file before `update_commands()` completes. If execution halts between these steps, `plugins.lock.json` might list registered opencode agents while commands remain unlinked. This state is safe to recover from. Because `gal refresh` is idempotent, running it again brings the system to a fully consistent state.

### Stale Skill Directories

The directory `~/.config/opencode/skills/` is tracked in `legacy_paths`. If GAL ownership is confirmed via lock-file attribution, a `.gal-managed` marker, or a verified symlink target, `remove_if_gal_owned_dir` removes it automatically. If the directory predates lock-file tracking and contains no ownership markers, the safety check leaves it untouched, causing `gal doctor` to report an unmanaged directory. Verify that the folder contains only outdated GAL projections, then remove it manually:

```powershell
Remove-Item -Recurse -Force "$env:USERPROFILE\.config\opencode\skills"
```

This safety check prevents accidental deletion of personal user files located in projected directories.

## Development Helper Skills

GAL includes several built-in skills to support code implementation and technical writing:

| Skill | Purpose |
| --- | --- |
| [`result-pattern`](plugins/gal-core/skills/result-pattern/SKILL.md) | Applying the Result pattern for expected operational failures while reserving exceptions for system faults. |
| [`structured-logging`](plugins/gal-core/skills/structured-logging/SKILL.md) | Structuring log fields, selecting appropriate log levels, and standardizing log output. |
| [`markdown-formatting`](plugins/gal-core/skills/markdown-formatting/SKILL.md) | Standardizing Markdown formatting and style across project documentation. |
| [`doc-coauthoring`](plugins/gal-core/skills/doc-coauthoring/SKILL.md) | Collaborating on technical proposals, specifications, and design documents with users. |
| [`doc-sync`](plugins/gal-core/skills/doc-sync/SKILL.md) | Maintaining the NDJSON structure map and detecting code-to-documentation drift. |
| [`local-first-search`](plugins/gal-core/skills/local-first-search/SKILL.md) | Querying local repository files first before consulting external notes or backends. |
| [`defuddle`](plugins/gal-core/skills/defuddle/SKILL.md) | Extracting clean Markdown from web pages to minimize token consumption. |
| [`skill-creator`](plugins/gal-core/skills/skill-creator/SKILL.md) | Developing, modifying, and evaluating reusable agent skills. |
| [`mcp-builder`](plugins/gal-core/skills/mcp-builder/SKILL.md) | Building and testing custom Model Context Protocol (MCP) servers. |
| [`install-gal`](plugins/gal-core/skills/install-gal/SKILL.md) | Installing and verifying the `gal` binary in new environments. |
| [`adversarial-review`](plugins/gal-core/skills/adversarial-review/SKILL.md) | Methodologies used by review roles to audit implementation plans, diffs, and architectural claims. |
| [`opencli-research`](plugins/gal-core/skills/opencli-research/SKILL.md) | Retrieving structured external data using OpenCLI to reduce token overhead. |
| [`text-flowcharts`](plugins/gal-core/skills/text-flowcharts/SKILL.md) | Documenting branching workflows and multi-step processes as monospaced text diagrams. |
| [`git-commits`](plugins/gal-core/skills/git-commits/SKILL.md) | Drafting Conventional Commit messages that conform to repository standards and creating commits. |

## Optional Local Review Tooling

The optional `security-guidance` plugin provides automated pattern matching, turn-by-turn diff analysis, and commit-triggered security reviews for Claude Code. The plugin operates advisory-only: it never blocks writes or commits, and it is not a required CI check. Contributor accounts absorb all token usage associated with plugin evaluations.

The plugin requires Python 3.7+ for basic hooks and Python 3.10+ for agentic evaluations. On initial execution, it provisions a virtual environment at `~/.claude/security/`. On Windows systems, verify that `python3` resolves to a valid Python installation rather than the Windows Store app alias. If the tool reports an issue related to a GAL trust boundary, cross-reference the findings with [SECURITY.md](SECURITY.md). Disclose vulnerabilities in releases privately using the security advisory process.
