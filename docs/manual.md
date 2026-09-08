# GAL User Manual

**English** · [日本語](i18n/ja/manual.ja.md) · [繁體中文](i18n/zh-Hant/manual.zh-Hant.md)

The GAL daily usage guide covers installation, initial repository setup, workflow operations, configuration, personalization, headless executor routing, and golem agents.

This manual details **GAL operation**. System architecture (codebase and `~/.gal/` topology, release lineage) is documented in [`docs/architecture.md`](architecture.md). Maintainer procedures (distribution mechanics, provider packaging, modification guidelines) are documented in the [developer guide](devguide.md). These topics are excluded from this manual.

## Overview

GAL uses `~/.gal/plugins/gal/` as the canonical plugin root. Provider-visible targets function as projections rather than content owners. Configuration is stored in `~/.gal/config/config.json`. Refer to [architecture → `~/.gal/` runtime layout](architecture.md#gal-runtime-layout) for `~/.gal/` layout and ownership boundaries.

The `gal` binary automatically locates its source root through a `.git`-bounded current working directory search followed by a binary-side packaged layout. The `devMode` and `galRoot` configuration keys are neither required nor read.

## Installing GAL

Select the appropriate platform installation option. After installation, execute `gal init` within the repository to generate repo-local adapters (`AGENTS.md` and `CLAUDE.md`) — refer to [First Run in Your Repo](#first-run-in-your-repo).

### Install Options

#### `cargo install --git` (from source)

```bash
cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli
```

GAL is excluded from crates.io — the `--git` flag is mandatory. The `gal-cli` package name is required within a Cargo workspace. The installed binary is named `gal`.

#### Homebrew (macOS / Linux)

```bash
brew install monkey1wizard/tap/gal
```

This is the standard package-manager installation method for macOS and Linux.

#### winget (Windows — probe first)

```sh
winget install Monkey1Wizard.GAL
```

Verify availability via `winget show Monkey1Wizard.GAL` before installation, as catalog visibility may trail releases. The `Get-Alias gal` shadow limitation applies here.

#### Direct download (release archive)

Download `gal-<version>-<platform>-<arch>[.zip|.tar.gz]` from the GitHub page, extract the contents, and add `gal` (or `gal.exe`) to the `PATH` (macOS / Linux: ensure execution permissions via `chmod +x gal`).

#### curl (Linux / macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | bash
```

The script verifies the SHA-256 checksum against `checksums.txt` (mandatory requirement, aborts on mismatch), followed by cosign keyless signature validation. Signature verification is best-effort regarding availability but strict regarding outcome. If cosign is absent or signature files fail to download, the installer issues a warning and proceeds using only the SHA-256 result. If cosign executes and reports a signature mismatch, the installation aborts before extraction or writing. The binary installs to `~/.local/bin` and the GAL source payload to `~/.local/share/gal`. Version overrides use `GAL_VERSION`.

#### irm (Windows)

```sh
irm https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.ps1 | iex
```

This method shares SHA-256 and cosign verification semantics with the curl path, including installation abortion upon signature check failure. Installs `gal.exe` and the GAL source payload directly into `%LOCALAPPDATA%\Programs\gal` (add this directory to the User `PATH`, the installer outputs the exact command). Version overrides use `$env:GAL_VERSION`.

Note: PowerShell's built-in `Get-Alias gal` can shadow the binary. If `gal` resolves to an alternative target, invoke the executable via its absolute path.

### Plugin Registration per Runtime

Plugin registration is the primary path across supported runtimes with a Markdown plugin container. All registered runtimes load from the single canonical root rendered at `~/.gal/plugins/gal/` by `gal refresh`. Runtimes lacking a Markdown plugin container (currently OpenCode alone) use file projection fallback instead.

To prevent duplicated skill or agent listings between plugin-provided assets and projected files, each runtime configuration ends with enabling `pluginMode.<runtime>` in `~/.gal/config/config.json` followed by `gal refresh`.

#### Claude Code

Claude Code registers the canonical root as a live skills-directory plugin:

1. Create a junction or symbolic link pointing `~/.claude/skills/gal` to the canonical root `~/.gal/plugins/gal`:
   - Windows (PowerShell):
     ```powershell
     New-Item -ItemType Junction -Path "$env:USERPROFILE\.claude\skills\gal" -Target "$env:USERPROFILE\.gal\plugins\gal"
     ```
   - macOS / Linux:
     ```bash
     ln -s ~/.gal/plugins/gal ~/.claude/skills/gal
     ```
2. Run `claude plugin list` and confirm that `gal@skills-dir` is listed and loaded. In this skills-directory mode, Claude Code loads directly from the junction in place without caching.
3. Enable Claude plugin mode in `~/.gal/config/config.json`:
   ```json
   {
     "pluginMode": {
       "claude": true
     }
   }
   ```
4. Run `gal refresh`. When `pluginMode.claude` is true, GAL skips writing command files to `~/.claude/commands/*.md` and prunes prior copies, avoiding duplicate entries alongside plugin skills.

The marketplace path remains secondary for snapshot consumers who install via `claude plugin add`, which copies files into `~/.claude/plugins/cache/<marketplace>/gal/<version>/`.

#### Codex

Codex registers GAL from the local plugins directory:

1. Register the local plugins marketplace directory:
   ```bash
   codex plugin marketplace add ~/.gal/plugins
   ```
   This reads `~/.gal/plugins/.agents/plugins/marketplace.json` generated by `gal refresh`.
2. Add the plugin:
   ```bash
   codex plugin add gal@gal
   ```
   Codex copies the plugin into `~/.codex/plugins/cache/gal/gal/<version>/`.
3. Run `codex plugin list` and confirm that `gal@gal` appears in the list.
4. Enable Codex plugin mode in `~/.gal/config/config.json`:
   ```json
   {
     "pluginMode": {
       "codex": true
     }
   }
   ```
5. Run `gal refresh`.

**Shared `~/.agents/skills` caveat:** Codex and OpenCode share `~/.agents/skills`. When OpenCode is not selected on the machine, setting `pluginMode.codex: true` skips projecting core skills to `~/.agents/skills` and prunes prior copies. When both Codex and OpenCode are selected on the same machine, GAL continues projecting core skills to `~/.agents/skills` so OpenCode remains operational, which means Codex will display both the plugin skills and the projected skills.

#### GitHub Copilot

GitHub Copilot consumes GAL as an Agent Plugins 1.0.0 package:

1. Register `~/.gal/plugins` as a directory marketplace in Copilot settings or configuration, which reads `~/.gal/plugins/.claude-plugin/marketplace.json`, and enable `gal`. Copilot loads the plugin live in place without copying, discovering agents from `com.github.copilot/agents/`, rules from `com.github.copilot/rules/`, and MCP configurations from `mcp.json`.
2. Run `copilot plugin list` and confirm that `gal` is enabled.
3. Enable Copilot plugin mode in `~/.gal/config/config.json`:
   ```json
   {
     "pluginMode": {
       "copilot": true
     }
   }
   ```
4. Run `gal refresh`. When `pluginMode.copilot` is true, GAL skips projecting agent files to `~/.copilot/agents/*.agent.md` and prunes prior copies. Copilot command skills remain projected under `~/.copilot/skills/` because Copilot does not load plugin `commands/`.

#### Antigravity

Antigravity installs GAL via local directory discovery:

1. Install the plugin from the canonical root:
   ```bash
   agy plugin install ~/.gal/plugins/gal
   ```
   This creates a junction at `~/.gemini/antigravity-cli/plugins/gal` pointing to the canonical root.
2. Remove duplicate `claude-code` import: If Antigravity previously imported plugins from Claude Code, `agy plugin list` reports two `gal` entries (`local-install` and `claude-code`). Because `agy` has no command to remove a specific import from `import_manifest.json`, open `~/.gemini/config/import_manifest.json` in an editor and remove the `claude-code` object from the `imports` array, keeping only the `local-install` entry. Confirm with `agy plugin list` that exactly one `gal` entry remains.
3. Enable Antigravity plugin mode in `~/.gal/config/config.json`:
   ```json
   {
     "pluginMode": {
       "agy": true
     }
   }
   ```
4. Run `gal refresh`. When `pluginMode.agy` is true, GAL skips projecting command skills to `~/.gemini/antigravity-cli/skills/` and prunes prior copies.

#### OpenCode (Projection Fallback)

OpenCode does not use a Markdown plugin container, as its extension model relies on JavaScript or TypeScript code modules installed via npm or local packages. OpenCode therefore uses GAL's file projection fallback instead of plugin registration. Running `gal refresh` projects native Markdown commands to `~/.config/opencode/commands/`, agents to `~/.config/opencode/agents/`, and core skills to `~/.agents/skills/`.

#### Chat-Driven Binary Installation (Secondary Marketplace Path)

Users discovering GAL through the Claude Code or Codex plugin marketplace snapshot branch can use the chat-driven setup:

1. Locate and install the **GAL plugin** within the Claude Code or Codex plugin marketplace (search "gal"), or add it from the [marketplace snapshot branch](https://github.com/monkey1wizard/golem-agents-legion/tree/marketplace-snapshot).
2. Request installation by asking the agent **"help me install gal"**. The plugin provides an `install-gal` skill that prompts for consent, executes the appropriate package manager (Homebrew, winget, or `cargo install --git`), verifies the binary, and runs `gal init` in an uninitialized repo (or `gal render-adapters` if the repo is already initialized).

The standalone marketplace plugin does not provide a complete GAL environment on its own. All workflow commands require the `gal` binary, making binary installation mandatory. Once `gal` is installed, follow the primary in-place registration sequences above for daily use.

---

Running `gal` without arguments prints usage instructions and exits with code 0. Unrecognized subcommands fail with exit code 64.

### Upgrading (`gal update`)

The `gal update` command outputs the installed version and platform-specific upgrade instructions. Self-updating is unsupported. Upgrades must use the original installation method:

```bash
cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli  # cargo
winget upgrade Monkey1Wizard.GAL                                                   # Windows
brew upgrade gal                                                                   # macOS / Linux
```

## First Run in Your Repo

### `gal init`

The `gal init` command generates two repo-local adapter roots (`AGENTS.md` and `CLAUDE.md`) from `gal-core` templates and scaffolds the `.dev/` directory (`.dev/project.md` and `.dev/state.md`) during initial execution on an uninitialized repository. If a repository is already initialized or half-initialized, `gal init` refuses to run, writes nothing, and exits with code 1.

| State | `.dev/project.md` | `.dev/state.md` | `gal init` | Exit |
| --- | --- | --- | --- | --- |
| Uninitialized | absent | absent | Fresh bootstrap. Writes both files from the templates, renders the adapters | 0 |
| Initialized | present | present | Error. Names `.dev/project.md`, points at `gal render-adapters` and at delete-then-init | 1 |
| Half-initialized | present | absent | Error. Names the file found and the file missing, points at version control or delete-then-init | 1 |
| Half-initialized | absent | present | Same, mirrored | 1 |

```
gal init: this repository is already initialized (.dev/project.md exists).
  To regenerate AGENTS.md and CLAUDE.md from .dev/project.md, run: gal render-adapters
  To start over from the templates, delete .dev/project.md and .dev/state.md, then run gal init again.
```

```
gal init: this repository is half-initialized: .dev/project.md exists but .dev/state.md is missing.
  Restore .dev/state.md from version control, or delete .dev/project.md and run gal init again.
  gal init does not overwrite .dev/project.md.
```

### `gal render-adapters`

The `gal render-adapters` command regenerates the repo-local adapters from `.dev/project.md` on an initialized repository.

| State | `.dev/project.md` | `gal render-adapters` | Exit |
| --- | --- | --- | --- |
| Initialized | present | Re-render `AGENTS.md`, `CLAUDE.md`, the conditional layers, prune retired roots. One row per path | 0 |
| Uninitialized | absent | Error. `gal render-adapters: .dev/project.md not found. Run gal init first.` | 1 |

`.dev/state.md` is never read or mentioned.

### Adapter Maintenance & `gal refresh` Migration

When maintaining an initialized repository or upgrading from earlier GAL versions that generated five adapter roots, running `gal render-adapters` reconciles repo-local adapters and applies prune-migration:

- **Pruning retired roots:** Three legacy bridge roots (`GEMINI.md`, `.github/copilot-instructions.md`, and `.agents/rules/gal.md`) are retired. During adapter generation or refresh, GAL inspects these retired paths. If a retired file carries the exact GAL generated marker on its first line, GAL deletes the file and reports it as `pruned (GAL-owned)`. If removing the file leaves `.agents/rules/` or `.github/` empty, the empty parent directory is also removed.
- **Preserving hand-owned files:** Any file at a retired root path that lacks the GAL generated marker is treated as user-authored. GAL preserves the file untouched and reports it as `kept (hand-owned)`.
- **Conditional layers:** If Rust conventions are enabled in `.dev/project.md`, the conditional layers (`.claude/rules/gal-rust.md` and `.github/instructions/gal-rust.instructions.md`) are updated alongside the two adapter roots. If disabled, any stale GAL-owned conditional layers are removed.

### `.dev/project.md` in your Project

The `.dev/project.md` file serves as the compressed project summary utilized for adapter rendering. Eight H2 sections are **strictly required once each**: `What This Is`, `Tech Stack`, `Architecture`, `Constraints`, `Response Style`, `Freshness`, `Project Language`, `Protected Paths`.

This requirement operates as fail-closed. Because `gal init` writes this file from the template, the check reports against a hand-edited `.dev/project.md`: missing or duplicated sections cause `gal render-adapters` to reject the entire render, identify the problematic heading, and halt adapter file writing. Errors report one missing heading per execution. Files missing multiple sections require iterative correction passes. Add the identified section manually (referencing `plugins/gal-core/templates/project.md` format), re-execute, and append subsequent sections.

The `.dev/project.md` file enforces a strict size limit. Rejections based on size require content trimming rather than execution retries. The rendering process strictly avoids writing partial adapter sets to bypass size constraints.

Below the `Tech Stack` table, `.dev/project.md` contains the `<!-- gal:authoritative-check -->` marker followed by a `json` code fence defining the `{"command": [...]}` shape. Each element in the `command` array is split on whitespace and executed verbatim with no shell interpolation. Because no working directory is set on spawned commands, each command inherits the working directory of the process that invoked `gal finalize-check` rather than automatically resetting to the repository root. For a documentation-only repo with no code command to run, the recovery path is setting the array to `["true"]`. The `gal finalize-check` gate consumes this authoritative check fence during final pipeline verification.

### `gal doctor`

The `gal doctor` command verifies local setup health, encompassing the binary, canonical root, runtime surfaces, and configuration. Execute this command after installation, after upgrading, or upon agent failure to detect GAL commands.

`gal doctor` checks include:

- **OpenCode projection drift** — detects when `~/.config/opencode/` files (commands and agents) no longer match what the canonical root would render. Reports warnings for stale content and errors for missing files. Run `gal refresh` to repair.
- **Claude skill surface** — reports a warning (not an error) when `~/.claude/skills/gal` is missing, with manual creation instructions. `gal refresh` does not create this surface automatically.

Within initialized repositories, the command additionally outputs a read-only adapter-size advisory table covering the two adapter roots (`AGENTS.md` and `CLAUDE.md`). Oversized adapters generate a `[WARNING]` finding rather than an error, preventing isolated run failures.

Regarding the opt-in self-test for headless coding agent execution, refer to [Executor Self-Test](#executor-self-test-gal-doctor---executor-smoke).

### Triggering GAL Commands per Runtime

GAL commands initialize via runtime-specific mechanisms, resulting in distinct triggers:

| Runtime | Trigger | Notes |
| --- | --- | --- |
| Claude Code | `/gal status` | native plugin command |
| Codex | `$gal-status` | exposed as a skill (`$` prefix, or `/skills`) |
| Copilot | `/gal-status` | exposed as a skill (`/skills list` to browse) |
| Antigravity | `/gal-status` | exposed as a skill (Antigravity lacks native `commands/` folders, commands are Agent Skills) |
| OpenCode | `/gal-status` | native Markdown command |

Note the syntax difference. Non-Claude runtimes utilize `gal-status` (hyphen) instead of `gal status` (space).

### When Command Changes Appear

Command update visibility depends on runtime loading mechanics:

- **Claude Code** loads in place as `gal@skills-dir` via the primary skills-directory junction with no cache, so command and skill updates appear immediately in new turns. (When using the secondary marketplace copy mode, restart Claude Code to reload updated plugins).
- **Antigravity** registers commands during startup. **Restart agy** to load changes.
- **Copilot / OpenCode** read command and skill files directly during each new session.
- **Codex** auto-detects skill changes within the active thread per documented primary behavior. **Restart Codex or initialize a new thread** as a fallback if changes fail to appear.

> **Support boundary:** GAL no longer projects commands or skills to Gemini CLI. Users of the retired Gemini CLI surface must migrate to Antigravity, whose command surface is `~/.gemini/antigravity-cli/skills/<name>/SKILL.md`.

Key Codex behaviors include:

- **Context-budget omission is not a failure.** Codex limits the initial skill list. Exceeding this limit causes description shortening followed by skill omission from the list. Omitted skills remain directly callable via `$skill-name`.
- **Same-name dual-listing.** Multiple tools projecting skills with identical names bypass Codex merging functionality. Both entries may appear in the skill selector.

## Running the Workflows

Workflow flowcharts and phase overviews reside in the [README](../README.md#how-gal-works). This section specifically details decisions required from the user, manual inputs, and required actions upon workflow interruption.

### Planning: Your Decisions

- The `/planning` command converts requests into source plans located at `.dev/plans/<type>-<slug>.md`. The planning phase supports collaboration, allowing the user to discuss, merge, or split plans freely and consult golem agents concurrently.
- **Open questions require resolution by the user.** The `/deep-planning` command mandates resolution of all `## Open Questions` entries prior to `/refining-plan` execution. Questions follow class-gating. **H**-class questions demand human-only authority, prohibiting agent closure. **A**-class questions allow architect role closure accompanied by recorded rationale. **F** denotes false questions. Ambiguous questions default to H-class classification awaiting user input.
- **Approval requires explicit confirmation.** Following `/refining-plan` convergence, record approval within the plan's `## Approval` section as four ordered lines: `- Human approval: [pending|approved]`, `- Architect review: [pending|clear|blocked|not-required]`, `- Design review: [not-requested|clear|blocked]`, `- Business review: [not-requested|clear|blocked]`. The `/plan-to-prompt` command requires the literal line `- Human approval: [approved]` and rejects execution prompt generation lacking it.
- **Multiple active plans demand target specification.** Concurrent active plans require explicit plan file designation during planning command invocation. GAL strictly avoids automatic selection.

### Pipeline: Start, Stop, Resume

- **Start:** Execute `/gal pipeline`. Single active plans trigger automatic execution prompt resolution. Multiple active plans require explicit prompt file specification (`.dev/plans/<slug>.prompt.md`).
- **Execution phase:** The pipeline iterates tasks autonomously following an implement, test, audit, commit, and handback-check sequence per task. After each task convergence gate, the handback checker decides whether the run may continue, stop at a retry ceiling, pause for a typed human-required blocker, or hand off to finalize. User intervention remains unnecessary unless that checker or a runtime cutoff says otherwise.
- **Fix retries require new authority and real implementation work.** After a failed test or audit, the pipeline writes one OPEN retry handoff and redispatches implement with `--fix`. A retry is refused before executor spawn when its task goal, handoff, affected-file allowlist, or agent contract is unchanged from the preceding attempt. If the executor completes without changing any affected implementation file, the round exits non-zero as `fix-round-no-change`; prompt, receipt, replay-sidecar, and executor-log writes do not count as implementation changes.
- **Interruption conditions:** The pipeline halts for human-decision blockers, typed human-required blockers from the handback checker, tasks reaching the retry ceiling (three failed verification attempts), head-drift or boundary-scope decisions, goal-gap failures, or configured working-hours hard stops. Runtime cutoff is recovery-only; it writes an interrupted-phase note and a rerunnable resume marker but does not authorize final output.
- **Resume protocol requires re-execution.** Resolve the blocker or underlying cause, then re-execute the identical `/gal pipeline` command. For a replay refusal, the recorded OPEN handoff must describe a substantively different problem or next action before resuming; changing timestamps or other display metadata cannot clear the refusal. Execution resumes from the recorded cursor after the next handback check succeeds. Completed tasks bypass re-execution.

### Test-first pipeline operations

Tasks marked `Pipeline Contract: test-first-v1` use a deterministic, receipt-backed lifecycle. The marker is emitted by the planning transition, not by an executor. A markerless prompt is `legacy`; a marked task is either `Test-first: required` or `Test-first: not-applicable`. Do not infer applicability from the presence of tests or from an agent's opinion.

#### Applicability, runner grammar, and safe paths

- **Required:** CODER may create only a behavior-free scaffold; TESTER runs the frozen probe and records the `Expected failures` red result; CODER implements without committing; ORCHESTRATOR reruns the exact same command for green; AUDITOR reviews the dirty tree; then ORCHESTRATOR performs the implementation commit.
- **Not-applicable:** CODER implements without a red probe; ORCHESTRATOR runs the correctness gate; TESTER runs the locked non-red probe; AUDITOR reviews the dirty tree; then the implementation commit occurs.
- **Legacy:** the existing implement → correctness gate → commit → test → audit order remains in force. A legacy task is not silently promoted to test-first.

The probe runner grammar is `gal test-first-probe run <plan> <task> <generation> <contract_digest> <phase> <expectation> <id> <selector> <argv_b64> <timeout_ms> [--env KEY=VALUE] [--expected-failure TEXT]`. All ten positional arguments are required and ordered; `argv_b64` is the base64url-encoded canonical child argv, not raw trailing arguments — there is no `--` argv terminator. `gal test-first-probe encode-argv <arg>...` is the sole sanctioned way to produce `argv_b64` from a plain argument list; no caller hand-builds the binary frame from `probe_evidence.rs`'s encoding format. The runner owns environment, timeout, stdout/stderr capture, exit status, and receipt, and derives the plan-scoped receipt path from `<plan>`/`<task>` itself. Never substitute a shell pipeline or hand-written receipt. `SKILL.template.md`'s §2d (Case 1 red, Case 2 non-red-pass) and §2f (green-rerun) name the exact `gal test-first-probe run` invocation and parameter sources inline — a marked task reaches `pipeline-converge-check: pass` following only those documented steps.

Production and test paths are two named lists, not two disjoint ones: a `Test-first: required` task may name the same file in both when the seam under test is private (a private `fn` is visible only inside its own file, so its unit test must live in that file's `mod tests` block). The freeze this enables is content-scoped, not file-scoped — CODER must not add, remove, or modify test items at the locked seam even in a file that is also a production path, and TESTER's mirrored freeze on production content works the same way. Every path is resolved beneath the repository root and validated as a `ValidatedRepoPath`; absolute paths, `..` traversal, symlinks, junctions, reparse points, and non-regular files fail closed. The validator is not a general filesystem sandbox: unrelated privileged processes can still race path components, so the consuming proof rechecks the identity immediately before use.

#### Canonical-root trust and observable-checkpoint threat model

The canonical source root (`plugins/gal-core/` in a source checkout, or the packaged canonical root selected by GAL) is the trust anchor for contracts. Provider-visible projections, executor prompts, receipts, and logs are derived evidence; they do not become a new authority. A higher-ranked corrupt root stops dispatch and never falls through to a lower-ranked root.

Evidence is accepted only at observable checkpoints: pre-spawn receipt preparation, child termination, receipt publication, transition write, boundary evaluation, audit, and commit verification. A timeout, wait I/O error, partial output, missing terminal record, or unconfirmed `termination` is an observation of uncertainty, not proof of success. The process lease serializes GAL dispatches for one receipt identity but does not defeat an unrelated privileged racer.

Hard links and racer substitutions are excluded from the trust claim. Existing link/reparse components, hard-link ambiguity, linked receipt targets, and non-UTF-8 targets are rejected. Unix and Windows identity checks use the full 128-bit identity available to the platform (device/inode on Unix; volume/file identity on Windows), rather than a truncated or path-only comparison.

A consuming operation reopens or consumes only after identity and regular-file checks pass again; any path transition invalidates the prior proof. This recheck is part of the consuming proof, not an optional optimization.

#### Transitions, generations, and evidence

Prompt/status changes are consuming path transitions produced by the transition writer. Each transition records the prior digest, next digest, generation, producer, and outcome. A marked transition producer compares prior and next contract-region digests after locking and before writing any journal or prompt, allowing changes only under `contract-change`. Transition journals are Git-tracked; only an executing plan's own `.dev/pipeline/journal/<slug>/transition.journal.tsv` is exempt from `gal boundary-check`'s allowlist at every boundary kind (`state-recording`, `post-test`, `post-audit`, `implementation-commit`). Lock, backup, receipt, snapshot, and other plans' journals receive no boundary exemption at any kind. The generation increments for a baseline restore or dispute recovery; an implementation defect retries in the same generation. Executors do not add, remove, or modify frozen test items at the locked seam, or edit prompt markers, journals, or receipts, to manufacture evidence — the freeze applies to the content at the seam, not to the whole file, so a same-file task still permits production edits in that file.

The evaluator binds red and green evidence to the same canonical argv, task, phase, generation, and identity. Expected failure means the required red probe failed in the specified way; `NotRun`, spawn failure, timeout, missing evidence, foreign evidence, or a non-canonical path is never a pass. A `started` terminal marker without a confirmed terminal record is an unterminated attempt and fails closed.

#### Disputes, recovery, cleanup, and commits

Only **ORCHESTRATOR** classifies a disagreement as `probe-defect`, `implementation-defect`, or `contract-ambiguous`. Probe defects and contract ambiguity restore the verified production baseline, increment the generation, and require fresh red evidence. Implementation defects retry in the same generation. Unknown or unresolved disputes stop at the plan/refining boundary; do not edit tests to force green.

AUDITOR reviews the exact uncommitted task diff. The implementation commit is delayed until correctness, test, and audit receipts pass. Executors never run `git commit` or `git push`.

A deterministic commit gate exists for this boundary. It verifies the clean index before staging, exact dirty-set and cached paths and hashes, base and parent commits, range diff, receipt digests, and post-commit cleanliness. **It is not wired in yet:** no contract invokes `gal test-first-commit run`, so today the orchestrator commits with a plain `git commit` and those checks do not run. Read the paragraph above as the delay rule the pipeline does enforce, not as a claim that the gate verified it.

Markerless prompts maintain legacy behavior with explicit no-binding semantics. The legacy sequence remains `implement → correctness gate → implementation commit → test → audit`, with no scaffold, red phase, or inferred test-first contract. Markerless prompts perform no transition-journal binding and create no journal. The `legacy-bootstrap` CLI producer and writer are removed; the transition reader retains historical compatibility for existing `legacy-bootstrap` rows, but writable transitions reject creating new bootstrap records. Markerless prompts are unbound and create no journal.

`test-first-cleanup` is plan-scoped, all-or-nothing, and crash-safe. Before any deletion, it pre-validates two optional roots (`.dev/pipeline/receipts/<slug>` and `.dev/pipeline/snapshots/<slug>`) and every item against closed cleanup ownership. Missing optional roots pass as recorded no-ops. Tracked files, non-cleanup ownership items, link or reparse ambiguity, or any pre-validation failure rejects with zero mutations and publishes a fail receipt. Safe cleanup quarantines roots, verifies same-parent relocation and child identities, and writes a deterministic pass or fail receipt before deletion. Never delete the whole `.dev/pipeline/receipts/.locks/` directory; inspect and remove only the exact stale lease after confirming no child or SSH helper remains.

Legacy adoption is a one-time human-verified bootstrap from clean tracked exact prompt bytes. It publishes a matching journal and digest; later prompt or status tampering triggers a boundary stop. Permission or sandbox denial is an environment stop, not a workflow result: rerun the exact GAL command after access is granted. Do not replace missing receipts with narrative, bypass a `NotRun` result, or treat cleanup uncertainty as successful termination.

### Finalize: What Lands and What Gets Deleted

The `/gal finalize` command orchestrates the closure of a completed plan. Preconditions enforce exhaustive task completion and verification, proven via a zero-trust machine receipt (`gal finalize-check`). `gal finalize-check` provides a read-only work-surface guarantee: it performs no mutations against checked repository work surfaces (adapters, source files, documentation, `.dev/project.md`, `.dev/state.md`, plan files, execution prompts). The only permitted filesystem mutation is the explicit receipt file written at the caller-selected path.

- **Landed components:** A top-down, requirement-by-requirement review runs once, in finalize's own runtime, over the whole branch before doc-sync. For each requirement the review evaluates four layers, L1 Truths, L2 Files, L3 Wiring, and L4 Trust boundaries, and writes the result under `## Review Results` as an `### Finalize Review <date>` table with one row per requirement plus a findings list. The STEWARD agent then extracts the plan's durable knowledge into `README.md` and `docs/`. A merge to main follows, including worktree teardown if applicable.
- **One conflict shape resolves itself.** When the merge to main conflicts and the unmerged path set is exactly `{.dev/state.md}`, finalize runs the internal `gal state-merge` resolver, which merges the plan-keyed tables row by row and stages the result. Exit 0 continues the landing. `STATE_MERGE: unresolved` stops with the repository proven byte-identical to before the attempt, and `STATE_MERGE: rollback-unconfirmed` stops and names `.dev/state.md` as possibly mutated, requiring inspection. Every other conflict shape keeps the unconditional stop. Mechanism and rejected alternatives: `docs/architecture.md`.
- **Deleted components:** Plan files within `.dev/plans/` undergo removal only after documentation is committed and a post-write hygiene check passes. This ensures knowledge transfer to the durable layer prior to file erasure.
- **Retained components:** A close-out row is written to `.dev/state.md` containing the date, plan, and landing commit. Every non-blocking finding recorded in the `### Finalize Review <date>` table is upserted into `.dev/state.md`'s `## Follow-ups` section, newest first, trimmed to the newest five rows, so those findings survive plan-file deletion. The `gal-last-good` tag is assigned to the landing commit.

### Recovering After a Failed Finalize Gate

Run `gal finalize-check` and read every row before taking action. Full-mode check rows, in this order, are `authoritative-command`, `naming-gate`, `sync-idempotency`, `finalize-mode`, `project-source-doc-existence`, `state-bound`, `contract-roster-parity`, `doc-link-resolution`, and terminal `working-tree-clean`, and each must be `pass`. That is nine rows in a repository carrying `plugins/gal-core/` at its root, or seven rows without it, because `contract-roster-parity` and `doc-link-resolution` apply only when that directory exists. No row repeats per task. The `sync-idempotency` check verifies read-only candidate-render determinism across two render passes rather than applying adapters to disk; candidate-vs-disk differences are reported as report-only disk drift (`in-sync`, `drifted`, `absent-on-disk`, `extra-marker-owned`, `filter-personalized`) and do not fail the gate. Only the `authoritative-command` row carries exit status or launch error, bounded `stdout`/`stderr`, raw byte count, and truncation flag. The terminal `working-tree-clean` row carries exit status, dirty-entry count, and status output size. Every other row carries a summary, so the absence of command fields on a `naming-gate` row is normal and not a defect. Hygiene-only receipt rows (`project-source-doc-existence`, `state-bound`, `durable-layer-commit`, `finalize-review-shape`, `contract-roster-parity`, `doc-link-resolution`, and terminal `working-tree-clean`) validate post-write clean state, plus the mechanical shape of the `### Finalize Review <date>` table, before plan file deletion. `NotRun` or missing evidence is a failure, not a pass.

Route the failure by class:

- An authoritative command/tool failure or a dirty tree requires a separate remediation plan. Do not repair it inside finalize.
- An empty or invalid cell in the `### Finalize Review <date>` table fails `finalize-review-shape` in the hygiene-only receipt. Re-run Sequence 1 to produce a shape-valid table, then repeat the hygiene-only check.
- Genuinely unchecked work with a non-`DONE` workflow returns to `/gal pipeline`. `DONE` plus unchecked or contradictory state is terminal corruption; stop and create the required human handback.

A failed finalize gate authorizes no automatic repair, commit, or evidence rewrite.

For terminal recovery, run this sequence only after the committed state is clean:

```powershell
gal.exe pipeline-preflight --terminal-reverify <execution-prompt-path>
# On a passing terminal-reverify receipt, the orchestrator runs goal-backward verification in process.
gal.exe pipeline-handback-check <execution-prompt-path>
gal.exe finalize-check <execution-prompt-path>
```

`terminal-reverify` is prompt-only and read-only. It requires `Workflow: DONE`, all tasks checked, a cleared current-task cursor, no open retry/interruption or human handoff, and a clean tree. It reuses marked transition-journal digest equality and records a bound marked terminal receipt chain. Diagnostic `terminal-binding` explicitly evaluates marked transition-journal digest equality and terminal receipt SHA-256 binding, while markerless prompts declare explicit legacy/no-binding semantics (no journal, no digest binding). It cannot dispatch implementation, testing, auditing, security work, or remediation; mutate the prompt; repair evidence; weaken a gate; or create a commit. Proceed only from a fresh handback receipt that authorizes finalization, then rerun the full finalize check.

### Agent Contract Resolution

Before dispatching a pipeline phase built from a prompt or source plan, `gal` locates the authoritative agent contract (`agents/golem-{implementer|tester|auditor}.agent.md`) and embeds its exact bytes into the task spec sent to the executor. No dispatched executor, local or over the SSH lane, is ever told to open a control-node-only contract path — the spec is self-contained.

**Resolution order (first source root wins):**

| Tier | `contract_source` | Root |
| --- | --- | --- |
| 1 | `workdir` | the canonicalized `--workdir` itself, or its direct `plugins/gal-core` |
| 2 | `ancestor` | the nearest ancestor of workdir recognized as a GAL source root |
| 3 | `exe-side` | the directory beside the running `gal` binary |
| 4 | `embedded` | materialized at `~/.gal/embedded-src` |

`workdir` outranks `ancestor`, `exe-side`, and `embedded`. This keeps a trusted local GAL checkout authoritative even when a packaged `gal` binary of a different version is also on PATH — a repo vendoring `plugins/gal-core/` always resolves its own contract regardless of the installed binary's version. If you maintain a vendored checkout, `contract_source` on the `Dispatch:` marker (see [Inspecting a Dispatch](#inspecting-a-dispatch)) is where to check for that version skew.

**Recovery differs by failure shape:**

- **Corrupt winning root** — the highest-ranked root was found, but its selected phase contract is missing, non-UTF-8, or unreadable. Dispatch stops before spawn with an error naming the tier and root. This never falls through to a lower tier, so fix the file at the named root rather than assuming a lower tier will cover for it.
- **All-miss** — no tier yields a usable root at all. Dispatch stops (exit 1) with every tier's outcome listed, plus both recovery routes: reinstall `gal` via the packaging channel, or run the command from a GAL source checkout.

**Raw/direct exception.** Raw `gal dispatch` and raw task-spec `gal pipeline` inputs never resolve or invent this provenance. Their markers and executor-log headers stay byte-compatible with the pre-provenance format — expect no `contract=`/`contract_source=` fields on these paths.

### Wrap-up vs Finalize

The `/gal wrap-up` command functions as a **pause**, rather than a landing. It compresses session handoff notes into the execution prompt, updates session continuity within `.dev/state.md`, and executes a commit. This enables any runtime to resume exactly at the pause point. The command closes zero items. Execute this command for mid-plan stops. Reserve `/gal finalize` strictly for completed plans.

## Configuration (`~/.gal/config/config.json`)

Machine-local values reside in `~/.gal/config/config.json`. Retained machine paths map to `galSkills` and working-hours to `workingHours`. Additional keys include `planLanguage`, `memoryHarvest`, and `executorRouting` (detailed in [Headless Executors](#headless-executors)). Strictly prohibit writing local values into tracked documentation, command templates, or source files.

| Placeholder | Meaning | Common use |
| --- | --- | --- |
| `<WORKING_HOURS_ENABLED>` | whether working-hours enforcement is active | opt-in wrap-up and hard-stop enforcement |
| `<WORKDAY_START>` / `<WORKDAY_END>` | preferred workday in `HH:MM` | Working Hours schedule / After Hours boundary |
| `<WRAP_UP_TIME>` / `<HARD_STOP_TIME>` | wrap-up and hard-stop in `HH:MM` | shutdown-window / stop-work behavior |
| `<GAL_SKILLS>` | absolute path to the machine-local GAL skills directory | git filters and machine-local skill projections |

### Secrets & MCP Overrides

The tracked GAL source in `plugins/gal-core/mcp.json` is the singular source for gal's own MCP servers. `gal refresh` copies it verbatim into the canonical `.mcp.json`, merging any personal servers from `~/.gal/local/mcp.json`. Separate override files are unsupported.

gal performs no placeholder substitution. A `${ENV_VAR}` written in the manifest is carried through unchanged and resolved, if at all, by the MCP host that loads the file — typically from the process environment. Set such variables in your environment, not in `config.json`.

Maintain conservative, machine-agnostic tracked entries for Playwright MCP. Implement local-only browser behavior parameters including headed mode, viewport and device emulation, storage-state paths, output directories, persistent profiles, and extension and CDP wiring via `${ENV_VAR}` placeholders within the tracked manifest. These resolve directly from `config.json`:

```json
{
  "servers": {
    "playwright": {
      "args": ["-y", "@playwright/mcp@latest", "--isolated", "--headless",
        "--storage-state", "${PLAYWRIGHT_MCP_STORAGE_STATE}",
        "--output-dir", "${PLAYWRIGHT_MCP_OUTPUT_DIR}"]
    }
  }
}
```

Configuration syntax for dual Postgres databases on a single machine:

```json
{
  "servers": {
    "postgres-app": {
      "type": "stdio", "command": "uvx",
      "args": ["postgres-mcp", "--access-mode=restricted"],
      "env": { "DATABASE_URI": "${POSTGRES_MCP_APP_URI}" }
    },
    "postgres-analytics": {
      "type": "stdio", "command": "uvx",
      "args": ["postgres-mcp", "--access-mode=restricted"],
      "env": { "DATABASE_URI": "${POSTGRES_MCP_ANALYTICS_URI}" }
    }
  }
}
```

Export matching values in the environment the coding agent runs under. The `${ENV_VAR}` placeholders in MCP configurations resolve at host load time, not by gal. Installed runtime MCP configurations retain user ownership — gal never writes them. Prevent committing storage-state files, persistent profiles, browser artifacts, and secret-like local files.

### Working Hours

Working-hours enforcement defaults to disabled. Configure `~/.gal/config/config.json` under the `workingHours` key to establish workday boundaries. Setting `enabled` to `false` maintains deactivation. The `workdayStart` and `workdayEnd` keys define the operational window. The `wrapUpTime` key initiates reminders and the shutdown sequence. The `hardStopTime` key triggers an absolute agent work refusal. These settings represent machine-local preferences rather than tracked repository policies.

### Plan Language

- The `planLanguage` configuration in `config.json` is machine-local and optional. It dictates the default output language for `.dev/plans/*.md` and `.dev/research/*.md` absent explicit directives. The resolution sequence follows explicit directive, then `planLanguage`, then prompt-language auto-detect, and finally an `en` fallback.
- The `PROJECT_LANGUAGE` tracked project metadata dictates the canonical language for primary documentation.
- Files matching `.dev/plans/*.prompt.md` remain strictly English to ensure cross-model stability.

Non-English `planLanguage` settings generate three-layer plans. Layer one is an English semantic draft located at `.dev/plans/<slug>.en.md` acting as the technical-meaning authority. Layer two is the localized source plan at `.dev/plans/<slug>.md` intended for reading and editing. Headings, paths, task IDs, and verdicts remain English while narrative prose adopts the localized language. Layer three is the English execution prompt. **Manual editing of the localized plan is fully supported**. Subsequent planning-stage commands detect edits and pause at a read-only reconcile step, merging manual intent back into the English draft prior to continuation. Edits bypass silent overwriting. Following `/plan-to-prompt` execution, the draft deletes, leaving exactly two tracked files per plan. An English `planLanguage` bypasses this entire mechanism.

### Provider-Memory Harvest

An opt-in mechanism transfers useful lessons surfaced by the coding agent during the current conversation into GAL tracked files, preventing data loss upon session termination.

- **Default-off, machine-local.** Enable the feature by setting `config.json#memoryHarvest.enabled` to `true`. Missing or false values maintain the disabled state.
- **Restricted scope.** GAL refrains from opening, listing, or searching the coding agent chat history or session files to identify candidates.
- **Repo-scoped and redacted.** Candidates require a direct link to a repository file. Entries undergo paraphrasing and redaction prior to visibility. Raw quotes, secrets, machine paths, and personal notes are excluded.
- **Mandatory approval.** Candidate approval occurs exclusively during `/gal wrap-up`. Rejected or unanswered candidates evade writing. Approved candidates enter the active plan handoff notes as provisional, advisory task memory.
- **Approval constraints.** Approval does not guarantee promotion. The `/gal finalize` command promotes approved candidates to `docs/` only after independent verification matches the standard for all promoted lessons.

## Personalization (`~/.gal/local/`)

Machine-local personal content resides under `~/.gal/local/` and projects to all agents via the canonical-root render. The `gal` binary reads `local/skills/`, `local/mcp.json`, and `local/conventions/` but **strictly avoids writing to or deleting these user-authored paths**. Cross-machine synchronization falls outside `gal` capabilities.

### Personal Skills / MCP / Conventions

**Layout:**

```text
~/.gal/local/
  skills/
    <skill-name>/
      SKILL.md          ← hand-placed personal skill (gal read-only)
  mcp.json              ← personal MCP servers (same format as plugins/gal-core/mcp.json, gal read-only)
  conventions/
    <lang>.md           ← personal coding-style convention file (gal read-only, see below)
```

**Enable:** Activation relies on presence rather than configuration flags. Placing a file at the personal root constitutes the opt-in action. Absent directories or files maintain render outputs byte-identical to Core-only renders.

**Projection rules:**

- Personal skills merge after core content. Personal skills featuring names colliding with core skills suffer silent skipping under a core-wins policy.
- Personal MCP servers merge into the canonical `.mcp.json` file. Personal servers featuring names colliding with core servers face skipping under a core-wins policy.
- The `gal doctor` command reports the count of personal skills, personal convention files, and core-collision skips.

### Coding-Style Conventions

GAL Core excludes owner-personal coding-style conventions. House styles propagate to downstream repositories via three distinct sources:

**Source 1 — personal convention file (always-on, per-repo).** Duplicate the included example from `plugins/gal-core/templates/csharp-convention.example.md` to `~/.gal/local/conventions/csharp.md` and modify the content to reflect the target style. A personal convention file is selected only when its stem is one of `csharp`, `typescript`, `javascript`, or `go`; any other stem is excluded. No cross-language stem exists. Owners relying on the old always-include behavior can recover by renaming their convention file stem to one of those aliases. Execute `gal render-adapters` within any target repository featuring a matching `Language` row in `.dev/project.md`. The system injects the file into the repository adapters mirroring the gal-core convention process.

**Source 2 — installed agent-plugin detection (read-only, zero setup).** Existing official language plugins installed within the coding agent require only a `gal render-adapters` execution in the target repository. GAL correlates the plugin skill name and origin against the repository `Language` row and renders a **Detected Language Skills** reference block containing the name and a persistent load instruction. GAL strictly avoids copying skill content and abstains from installing, updating, or removing plugins. Newly installed plugins require a subsequent `gal render-adapters` execution for detection.

**Source 3 — personal skill (on-demand).** Author a `SKILL.md` file containing a clear description under `~/.gal/local/skills/<name>/`. Restart Claude Code to initialize detection, whereas other agents detect the file during their next read cycle. Agents load this source exclusively when named, contrasting with the always-on injection of sources 1 and 2.

**Source selection strategy:** Prioritize source 2 given existing official plugin installations to achieve zero setup. Source 1 provides always-on personal styling with maximum control. Source 3 serves on-demand agent consultation requirements.

**Miss contract:** Repository `Language` rows lacking matching sources result in adapters devoid of language-specific conventions or reference blocks. This behavior constitutes a silent, by-design skip rather than an error condition.

Insert a `| Personal Conventions | off |` row into the `.dev/project.md` Tech Stack table of a repository to disable sources 1 and 2. This practice is recommended for public repositories to prevent tracked adapters from embedding owner-machine content. Source 3 remains unaffected.

### Command-Skill Local Overlays (`SKILL.local.md`)

To implement machine-local customization for a command skill, generate the file `plugins/gal-core/commands/<command>/SKILL.local.md`:

- The file is gitignored and operates as user-owned machine-local input.
- The bake process appends `SKILL.local.md` into the generated `SKILL.md`.
- Avoid direct edits to `plugins/gal-core/commands/<command>/SKILL.md`. It is a generated file subject to replacement.
- Restrict `SKILL.local.md` to supplemental instruction content. Exclude secondary frontmatter blocks.

### local-notes Routing

External notes represent machine-local and optional components. GAL isolates repo-owned state from user-owned notes. Core behavior operates independently of private note stores. The portable binding contract resides in [`optional-capabilities.md`](../plugins/gal-core/conventions/optional-capabilities.md). It is app-agnostic, defaults to off, and executes exclusively upon machine-local backend readiness.

Absent, unreachable, or uninitialized backends trigger degradation to the standard repo-local workflow without generating failure alerts. Only opted-in and reachable backends permit reading, searching, or writing where explicitly authorized by the workflow. State is never mandatory. Repositories lacking note backends operate identically to fully connected instances, differing only by the absence of an optional context source.

Documented backend examples include Obsidian (`coddingtonbear/obsidian-local-rest-api`), Logseq (`ergut/mcp-logseq`), Joplin (`joplin-mcp`), generic markdown vaults (`vault-mcp`), and CJK-first retrieval (`SeekLink`). This list does not guarantee feature parity.

Repo-owned research defaults to the `.dev/research/` directory.

## Headless Executors

GAL supports offloading pipeline phases to a secondary headless coding-agent CLI, bypassing the conversation loop execution. Implement this configuration beneath `executorRouting` within `~/.gal/config/config.json`.

### Headless Executor Routing

The `config.json#executorRouting` key categorizes roles **by consumer** into three distinct objects. The `pipeline` object handles headless dispatch of implement, test, and audit phases. The `planning` object manages Codex native-subagent model selection for planning-stage review roles and strictly avoids headless dispatch. The `research` object is optional and routes at most two of the three parallel research workers. A shared `executors` default-model block accompanies these objects:

```json
{
  "executorRouting": {
    "executors": {
      "claude":   "claude-haiku-4-5-20251001",
      "codex":    "gpt-5.4-mini",
      "opencode": "opencode/minimax-m3-free",
      "copilot":  "claude-haiku-4-5-20251001",
      "agy":      "gemini-2.5-flash"
    },
    "pipeline": {
      "CODER":   { "executor": "codex" },
      "TESTER":  { "executor": "opencode" },
      "AUDITOR": { "executor": "claude", "model": "claude-sonnet-4-6" }
    },
    "planning": {
      "ARCHITECT": { "executor": "codex", "model": "gpt-5.4", "effort": "high" },
      "ANALYST":   { "executor": "codex", "model": "gpt-5.4-mini" }
    },
    "research": {
      "1": { "executor": "codex", "model": "gpt-5.4-mini" },
      "2": { "executor": "opencode" }
    }
  }
}
```

All three groups enforce closed role allowlists. The `pipeline` list includes `{CODER, TESTER, AUDITOR}`. The `planning` list includes `{ARCHITECT, ANALYST, DESIGNER, RELEASER}`. The `research` list includes only the numbered keys `"1"` and `"2"` — both optional, and the group itself is optional. Roles or keys assigned to incorrect groups, or unrecognized entirely, trigger skipping and generate warnings specifying the correct group. The legacy flat structure featuring role keys directly beneath `executorRouting` is retired, producing visible warnings without fallback mechanisms.

**executors block:** Defines the default model per tool. Role entries lacking a model specification inherit the default from `executors[executor]`. Explicit model declarations on a role supersede defaults.

**Per-role effort key:** Provides an optional reasoning-intensity indicator which maps to the native reasoning flag of each executor prior to spawn:

| Executor | `effort` → native flag |
| --- | --- |
| claude | `--effort <value>` |
| codex | `-c model_reasoning_effort="<value>"` |
| copilot | `--reasoning-effort <value>` |
| opencode | `--variant <value>` |
| agy | `--effort <value>` |

The accepted `effort` values per executor are:

| Executor | Accepted `effort` values |
| --- | --- |
| claude | `low`, `medium`, `high`, `xhigh`, `max` |
| codex | `none`, `minimal`, `low`, `medium`, `high`, `xhigh`, `max` |
| copilot | `none`, `minimal`, `low`, `medium`, `high`, `xhigh`, `max` |
| opencode | provider-specific |
| agy | `low`, `medium`, `high` |

GAL validates argv safety only and never validates the executor-specific vocabulary before dispatch.

Antigravity (`agy`) enforces an exclusive-or rule for reasoning effort: for a model supporting effort tiers, the reasoning intensity must be specified exactly once — either as an effort-suffixed model slug (such as `gemini-3.7-flash-medium`) or via `--effort <value>` with a base slug (such as `gemini-3.7-flash`), but never both and never neither. Specifying a base slug without an effort setting results in an agy error (`requires --effort`), whereas passing `--effort` alongside an effort-suffixed slug produces agy's conflict error (`conflicts with --effort`). Models without effort tiers reject `--effort` in every form.

Because a role without its own `model` inherits `executors.agy`, this exclusive-or rule applies to every agy route. Valid configurations must use one of two legal shapes across all agy routes:

1. **House shape (recommended):** Every agy role carries an explicit `effort` while `executors.agy` stays a base slug (for example `gemini-3.7-flash`), matching the medium-first default.
2. **Legal alternative:** `executors.agy` carries an effort-suffixed slug (for example `gemini-3.7-flash-medium`) and no agy role sets `effort`.

Mixing the two shapes (setting an explicit `effort` on a role while `executors.agy` uses an effort-suffixed slug) produces agy's conflict error (`conflicts with --effort`).

The mechanism operates fail-closed. Executors incapable of honoring the effort hint or encountering malformed values will degrade the dispatch prior to spawn instead of silently ignoring the hint. The `effort` key functions exclusively as a hint and never operates as a model selector or implies a paid or named model.

**Per-role `timeoutSecs` key:** An optional positive integer overriding the dispatch timeout for that role, applied identically to local and SSH (remote) routes:

```json
"pipeline": {
  "CODER": { "executor": "codex", "timeoutSecs": 900 }
}
```

A role that omits `timeoutSecs` keeps the unchanged 300-second CLI default. A role that sets `0` is rejected with a warning at load time and behaves as if the key were absent — zero is never a meaningful timeout. Setting `timeoutSecs` changes no `dispatch-script` command-line grammar; it only changes the timeout value the bin applies once routing resolves.

**Adapter behavior notes:**

- **OpenCode** dispatches using `--agent build` to enable the write-capable agent, bypassing the read-only default that silently blocks writing. It also utilizes `--auto` as the permission-bypass flag.
- **Copilot** appends `--no-custom-instructions` and `--disable-builtin-mcps` to prevent headless prompt-mode from overflowing context limits. Copilot Free operates strictly as **auto-only**, maintaining `model: auto` and local-only execution.

**Role table:**

| Role | Group | Purpose |
| --- | --- | --- |
| `CODER` | pipeline | Writes implementation code following a plan. Should differ from `TESTER` when practical |
| `TESTER` | pipeline | Writes tests from plan spec and public API only. Should differ from `CODER` when practical |
| `AUDITOR` | pipeline | Audits deep performance and security. Should differ from `CODER` when practical, tier >= `CODER` |
| `ARCHITECT` | planning | Adversarial plan review covering trade-offs, over-engineering, and bugs |
| `ANALYST` | planning | Business logic review covering ROI, domain correctness, and user impact |
| `DESIGNER` | planning | UX, UI, and DevEx review |
| `RELEASER` | planning | Release-flow design (read-only) |
| `RESEARCHER` | research | Investigates a research question as one of three parallel, mutually blind workers (`RESEARCHER#0`/`#1`/`#2`) |

**The optional `research` group.** `/gal research` and `/gal deep-research` each spawn three parallel `RESEARCHER` workers, numbered `#0`, `#1`, and `#2`. The `research` object under `executorRouting` accepts only the numbered keys `"1"` and `"2"` — never `"0"`. Both keys are optional, and the whole `research` group is optional. Comparing the three briefs, adjudicating claims, and writing the final document stay ORCHESTRATOR-owned and in-process regardless of how the three workers resolve — routing affects only where each worker's own research runs.

- **`#0` is fixed.** `RESEARCHER#0` is always a native subagent. It has no config key and is never looked up in `executorRouting` — there is no `research."0"` entry to set.
- **`#1` and `#2` resolve independently, each by the same three-branch rule, in this priority order:**
  1. An explicit `research."1"` (or `research."2"`) entry, if present, wins.
  2. Otherwise, the `executors.agy` preference applies: the worker routes to `agy` using `executors.agy` as its model, with no `effort` set on the route.
  3. Otherwise, the worker falls back to a native subagent, same as `#0`.
- **Interaction with the agy exclusive-or rule ([above](#headless-executor-routing)):** when branch 2 resolves a worker to `agy`, that route deliberately carries no `effort` key. The exclusive-or rule still applies to it, so `executors.agy` itself must supply the reasoning intensity — as an effort-suffixed model slug (for example `gemini-3.7-flash-medium`). If `executors.agy` is instead a base slug with no suffix, the agy route fails with the standard `requires --effort` error, because nothing on this branch is allowed to set `--effort` explicitly.

**Zero-config outcome:** with no `research` group configured at all, all three workers — `#0`, `#1`, and `#2` — resolve to native subagents. A repository need not touch `executorRouting` to use `/gal research` or `/gal deep-research`.

> ⚠️ **SECURITY WARNING — bypass-permission.** Headless executor adapters trigger secondary CLIs using `--dangerously-skip-permissions` (Claude Code, Antigravity/agy), `--auto` (OpenCode), `--allow-all` (Copilot), or `-s workspace-write` (Codex). This grants **full trust** over the local filesystem and terminal, neutralizing sandbox protections. Activate executor routing strictly on trusted machines and environments. Prohibit activation when the repository or agent contracts originate from untrusted sources. Specifications forbid secondary CLIs from executing `git commit` or `git push`, representing an instruction rather than a technical enforcement.

### Remote Execution (SSH Dispatch Lane)

Cross-machine execution functions as a property of the resolved route rather than a separate command. The `/gal pipeline` command maintains identical behavior regardless of `CODER`, `TESTER`, or `AUDITOR` resolving to local or remote invocations. Append `sshTarget` and `remoteWorkdir` to a pipeline role entry to execute that phase via SSH:

```json
"pipeline": {
  "TESTER": {
    "executor": "claude",
    "model": "claude-sonnet-4-6",
    "sshTarget": "user@build-box",
    "remoteWorkdir": "/home/user/gal-remote"
  }
}
```

Both fields mandate concurrent presence. Partially configured entries load with warnings and trigger loud failures during dispatch.

**Prerequisites** — configure these directly on the remote machine, as GAL provides zero provisioning:

- Passwordless SSH access to the target (`BatchMode=yes`). Interactive password or passphrase prompts qualify as unreachable states and prevent retries.
- The routed agent CLI (`claude`, `codex`, `agy`, `opencode`) requires installation and authentication on the remote machine. The `gal` binary remains unnecessary on the remote target.
- The `git` repository in `remoteWorkdir` must match the control node checkout commit and feature a clean working tree prior to every dispatch.

**Expected result:** The dispatch executes synchronously over the maintained SSH session, matching local dispatch lifespans. Following a successful mutating implement phase, GAL retrieves the uncommitted remote diff and applies it to the control node. GAL resets the remote checkout (`git reset --hard && git clean -fd`) to a clean state strictly after a successful local application, prohibiting reverse execution sequences. Local application failures leave the remote checkout unmodified for inspection purposes.

The `remoteWorkdir` must function exclusively as a **GAL-dedicated checkout**. Avoid targeting checkouts utilized interactively, as the post-application cleanup process operates destructively by design.

**Known limitations (v1):** The copilot agent resists remote routing because its CLI-flag specification delivery resists forwarding across SSH command lines. Such routes trigger loud failures instead of silent degradation. Windows remote targets remain unsupported as the composed command demands a POSIX login shell. Disconnect-survival features are absent, causing dropped SSH sessions to fail the dispatch without reconnection or resumption options. The remote lane operates in a single-dispatch and manually-re-synced mode. The control node HEAD advances post-task commit while the remote checkout halts at the final sync point. Subsequent remote dispatches in the active run trip the guard until manual checkout synchronization occurs.

Confirm readiness bypassing live pipeline tasks utilizing the [Remote (SSH) Self-Test](#remote-ssh-self-test---transport-ssh) detailed below. The accompanying status table documents remote-only failure conditions and recovery procedures.

### Inspecting a Dispatch

Each dispatch generates **two** durable traces. Select the appropriate trace for the task:

**Tier 1 — GAL executor log (the audit trail, uniform across all tools).** Each execution lands in `.dev/executor-logs/<plan-slug>/` or the unscoped `.dev/executor-logs/` for direct dispatch. Files follow the `<timestamp>-<attempt>-<task>-<phase>-<executor>.log` naming convention. Headers document terminal state, exit code, actual model, git branch and HEAD, alongside the provider `session_id`. The `---STDOUT---` block captures the complete provider event stream, covering agent messages, command executions, file modifications, and token consumption metrics. Read this log to audit executor actions. The format remains identical across all five tools. Provider-local chat history serves advisory purposes, whereas this log constitutes the official repo-owned record.

**Terminal state vocabulary.** The log header classifies dispatch completion into terminal states:

| Terminal State | Meaning |
| --- | --- |
| `completed` | Process exited 0 with confirmed receipt delivery and expected workdir changes. |
| `no-receipt` | Process exited 0, but the expected receipt file was missing, empty, or unconfirmed. |
| `workdir-escape` | Retained for historical log vocabulary and match exhaustiveness only; not produced by the production dispatch path. |
| `no-writeback` | Process exited 0 and delivered a receipt, but produced zero modifications in the assigned git worktree under a porcelain-plus-content comparison. |
| `timeout` | Process terminated because the configured timeout elapsed. |
| `disconnected-partial` | Process exited with a non-zero status code. |
| `unavailable` | Routed executor CLI binary was not found on PATH. |

The `no-writeback` state has key operational boundaries:
- **Phase scope:** `no-writeback` applies strictly to `scaffold` and `implement` phases. It excludes `audit` and `test` phases because those phases deliver their entire contracted output through gitignored receipt files (`.dev/pipeline/receipts/`), which a git worktree snapshot cannot see.
- **Same-flag boundary:** When tracked or untracked files retain identical `git status --porcelain` flags before and after dispatch (such as pre-dirty files), content changes are detected through SHA-256 content digest comparison so genuine deliveries are not classified as `no-writeback`.
- **Gitignored-write boundary:** A phase whose only writes land on gitignored paths is reported as `no-writeback` even though the executor did work, because the worktree status snapshot tracks repository changes and ignores gitignored paths.
- **Delivery contract coupling:** This classifier encodes today's phase-delivery contract (where `scaffold` and `implement` deliver via tracked repository files, while `audit` and `test` deliver via receipts). Changing how a phase delivers requires revisiting this classifier.

**Reading the `contract`/`contract_source` fields.** Pipeline-created dispatches for prompt/source-plan inputs append `contract=<control-node-abs-path> contract_source=workdir|ancestor|exe-side|embedded` to the `Dispatch:` marker line and to both the started and terminal headers of the executor log — the same coupled value, so any one of the three tells you which agent contract the executor actually ran against and from where. `contract_source=workdir` or `ancestor` means a local GAL checkout supplied the contract (check that checkout if behavior looks stale); `exe-side` means the packaged binary's own bundled copy; `embedded` means the fallback materialized copy at `~/.gal/embedded-src`. Raw `gal dispatch` and raw task-spec `gal pipeline` runs omit these fields entirely — their absence is expected, not a defect.

**Reading the provenance-gated `effort` field.** For pipeline-created dispatches only (provenance present), every success/degrade marker in the run carries a sanitized ` effort=<value|(default)>` field immediately before the `contract`/`contract_source` suffix, computed once after route resolution and reused unchanged for the rest of that run. Raw/direct dispatch and the `no-routing` degrade (no route was resolved, so no provenance) never carry an `effort` field — their markers stay byte-identical to the pre-effort format. When routing produces an `OFFLOAD` block, `gal dispatch-script` also pre-renders a `REPORT_LINE` field (`Dispatched: <phase[ (fix)]> <T-NN> - <ROLE> as <executor>, model <model>, effort <effort>`) that the orchestrator announces exactly once, verbatim, at each dispatch point.

**Tier 2 — provider-native session resume (for continuing/branching).** Log headers record a resumable `session_id`. Utilize this identifier to extend the conversation within the native provider UI. Commands **lack** uniformity:

| Executor | Native-view / resume command | Default visibility of headless session |
| --- | --- | --- |
| **claude** | `claude --resume <session_id>` | listed |
| **codex** | `codex resume <uuid>` (UUID bypasses the filter) | **hidden** — use `codex resume --include-non-interactive` (add `--all` to disable cwd filtering) to see it in the picker |
| **opencode** | `opencode run -s <session_id>` (continue) · `opencode export <session_id>` (dump JSON) · `opencode session list` (browse) | listed |
| **copilot** | `copilot --resume=<session_id>` | stored in `~/.copilot/session-store.db`, resume by id (no public list command) |
| **agy** | `agy --conversation <uuid>` | stored under `~/.gemini/antigravity-cli/brain/<uuid>/`, no list subcommand — browse by id |

**General guideline:** Audit dispatches by reading the Tier-1 executor log. Resume operations within the native tool using the corresponding Tier-2 command. The codex tool uniquely hides headless sessions by default based on intrinsic behavior rather than GAL configuration.

### Executor Self-Test (`gal doctor --executor-smoke`)

The standard `gal doctor` command avoids calling headless coding-agent CLIs. The `gal doctor --executor-smoke` variant constitutes an opt-in, repeatable self-test executing the identical headless dispatch path utilized by live pipeline tasks across all five supported CLIs: `codex`, `claude`, `copilot`, `agy`, `opencode`.

```sh
# Self-test all five agents (default --transport local)
gal doctor --executor-smoke

# Filter to specific agents (repeatable)
gal doctor --executor-smoke --executor codex --executor claude

# CI/scheduler use: machine-readable JSON, non-zero exit on any non-pass row
gal doctor --executor-smoke --json --strict

# Bound the per-agent timeout (seconds, default 300)
gal doctor --executor-smoke --timeout 60
```

This command triggers the authentic real-dispatch code path utilizing a synthesized routing file isolated from the live `config.json#executorRouting` configuration. It executes a minimal task designed exclusively to write a one-line receipt. Runs persist within a gitignored, timestamped directory defaulting to `.dev/executor-smoke/runs/<utc-run-id>/local/`. Outputs include a JSON report, a human-readable table, per-agent logs, and receipts. The `.dev/executor-smoke/latest.json` file persistently points to the most recent run.

**Status definitions:**

| Status | Meaning |
| --- | --- |
| `PASS` | Real dispatch completed and the receipt was verified. |
| `NOT_INSTALLED` | The CLI is not on PATH. Install it. |
| `NOT_AUTHENTICATED` | Confirmed unauthenticated. Run the tool's login command. |
| `AUTH_UNKNOWN` | Readiness could not be confirmed. A bounded call is attempted and actual outcome reported. |
| `UNSUPPORTED` | An `--executor` name outside the five supported agents. Never dispatched. |
| `CONFIG_ERROR` | A pre-dispatch problem (often an unsafe `--report-dir`). Caught before any write. |
| `CALL_FAILED` | The executor exited non-zero. Inspect the run executor log. |
| `NO_RECEIPT` | The executor exited 0 but never wrote the receipt. Exit 0 alone never constitutes success. |
| `TIMEOUT` | The bounded timeout elapsed. Widen `--timeout` if the tool is slow or investigate hung interactive prompts. |

Omitting the `--strict` flag forces a 0 exit code upon run completion. The report serves as the absolute source of truth rather than the exit code. This tool operates as a repeatable CLI command rather than a background service. Execute it manually, integrate it into CI pipelines, or trigger it via OS schedulers.

#### Remote (SSH) Self-Test (`--transport ssh`)

This self-test validates coding-agent readiness on remote machines over SSH. It maintains identical report schemas and statuses while altering the transport mechanism. Installation and authentication probes execute **on the remote machine** and resist inference from the control node.

```sh
gal doctor --executor-smoke --transport ssh --ssh-target <ssh-target> --remote-workdir <dedicated-checkout>

# CI/scheduler use
gal doctor --executor-smoke --transport ssh --ssh-target <ssh-target> --remote-workdir <dedicated-checkout> --strict --json
```

The `--ssh-target` variable defines an SSH host supporting non-interactive, key-based access (`BatchMode=yes`). The `--remote-workdir` variable is **mandatory** and specifies a dedicated GAL checkout matching the control node git HEAD with a clean status. Generated reports output to `.dev/executor-smoke/runs/<utc-run-id>/ssh/`.

**Remote-specific statuses:**

| Status | Meaning | Remediation |
| --- | --- | --- |
| `SSH_UNREACHABLE` | No non-interactive SSH session could be opened (checked before any executor probe). | Check the target, network, and key-based auth. |
| `REMOTE_GUARD_FAILED` | The remote workdir is missing, unsafe, dirty, or not at the control node HEAD. | Re-sync the dedicated remote checkout to the control node HEAD and ensure a clean state. |
| `REMOTE_FETCH_FAILED` | The remote process may have run but fetching its receipt back failed. | Inspect the run executor log and the remote receipt path. |

Remote copilot reports generate an `UNSUPPORTED` status in accordance with remote limitations. This state avoids pass counts and omission. The `PASS` status strictly demands terminal completion **and** a successfully fetched, non-empty receipt.

#### Recovering Receipt Lease Failures

Receipt leases prevent two GAL dispatches from authenticating each other's deterministic receipt path. Treat `reason=receipt-preparation-failed` or `reason=remote-receipt-freshness-failed` as a possible active-or-stale lease, not as permission to delete the receipt broadly. Read stderr for the exact local `.dev/pipeline/receipts/.locks/<hash>.lock`; remote freshness stderr includes `lock=.dev/pipeline/receipts/.locks/<hash>.lockdir`, whose `owner` file is the cleanup authority. Before removal, independently confirm that no matching GAL/SSH executor is still active—especially after a timeout or wait I/O error, because those outcomes do not themselves prove termination. Then remove only that named lock file, or the named remote `owner` file followed by its now-empty lock directory. Never clear the whole `.locks/` directory.

`remote-receipt-fetch-failed` means the post-run source was missing, empty, linked, non-regular, or could not be fetched; `remote-receipt-fetch-timeout` means the bounded fetch expired; `remote-receipt-fetch-read-failed` means a pipe read error left partial stdout that GAL refused to authenticate; `remote-receipt-fetch-too-large` means the helper drained the source but refused to retain more than its 1 MiB stdout cap. Stderr has the same retention cap. A `remote-receipt-fetch-*-unconfirmed` or `remote-receipt-lease-cleanup-*-unconfirmed` reason means the local auxiliary SSH process did not provide confirmed exit evidence, so its capped pipe drains were not joined. `receipt-lease-cleanup-failed` means checked local/control-side lock removal failed on a normal or early-return path; use the exact stderr path and keep the result downgraded. When cleanup accompanies a primary guard/log failure, the marker joins reason tokens with `+` or the returned error text names both; investigate both. `remote-receipt-lease-cleanup-failed` and `remote-receipt-lease-cleanup-timeout` mean owner-token cleanup was not confirmed, so the lock intentionally remains fail-closed even when the executor had already failed. An executor's own exit 73 remains exit 73 and is not a freshness failure unless the wrapper's dedicated freshness sentinel is present.

#### Recovering Phase Write-Back Failures

For in-scope dispatches (`audit` on any prompt, `test` on a markerless prompt), an executor's terminal state of `completed` (exit 0) represents receipt delivery to `<task>-<phase>.receipt.md`, not phase completion or prompt placement. The `gal` control node owns prompt placement and runs a fail-closed semantic write-back gate after dispatch completes.

**Observable failure:** If payload validation or rendering fails, `gal` prints `phase-writeback semantic failure for task <T-NN>` to stderr, appends a structured error record to the pipeline loop log, exits non-zero, and leaves the execution prompt file byte-identical on disk (the **unchanged-prompt guarantee**).

**Scope & exceptions:**
- Both local and SSH execution routes process receipt payloads through the same control-node semantic gate and produce identical deterministic prompt placement.
- `PipelineInput::RawSpec` and direct `gal dispatch` bypass the semantic write-back gate and remain receipt-only because no authoritative execution prompt file exists in those modes.

**Common causes & recovery protocol:**
- **Stale projected agent contracts:** If an executor emits malformed receipt content (such as extra unfenced headings or missing verdict markers) or attempts direct prompt edits, running `gal refresh` updates the repo-local and projected agent contracts to match current rules.
- **Malformed receipt payload:** Inspect the receipt file (`.dev/pipeline/receipts/...`) or the pipeline loop log for specific validation errors (e.g. missing `### [T-NN] YYYY-MM-DD` header, extra H2/H3 headings, or task ID mismatch).
- **Repair & rerun:** Run `gal refresh` if contracts were stale, resolve the receipt or dispatch issue, and rerun `/gal pipeline`. Operators should never relocate or copy-paste Markdown subsections into the execution prompt file by hand; binary placement ensures deterministic ordering under the correct H2 (`## Test Results` or `## Review Results`).

### Dispatched Worker Boundary and Project-Instruction Isolation

When `/gal pipeline` dispatches a task phase to a headless executor, the secondary coding agent executes in a worker role rather than an orchestrator role. To prevent dispatched workers from interpreting repository-level instructions (such as workflow obedience rules or orchestrator-gate requirements) as commands to run pipeline control commands, GAL establishes role isolation through two complementary mechanisms: a task-spec boundary and adapter-level project-instruction suppression.

#### Dispatched Worker Boundary

Every rendered task spec embeds a `## Dispatched Worker Boundary` block (formerly `## IMPORTANT: Orchestrator Gate Boundary`) placed prominently after the metadata separator and before `## Task Goal`. This section:
- Identifies the worker as a single scoped phase executor rather than the orchestrator.
- Explicitly prohibits invoking any `gal` subcommand (including `gal pipeline-preflight`, `gal pipeline-handback-check`, `gal boundary-check`, `gal pipeline-converge-check`, or `gal pipeline`), because all required pipeline gates are owned and satisfied by the orchestrator.
- Explicitly prohibits loading or executing any workflow `SKILL.md` file.
- Overrides any repository-level instructions or skill instructions that direct the agent to run orchestrator workflows. Dispatched workers must follow only the scoped task goal, the file allowlist, and the embedded `## Agent Contract`.

#### Project-Instruction Isolation Matrix

Depending on the executor CLI, repository-level instruction files (`AGENTS.md` or `CLAUDE.md`) may be loaded automatically into the model's instruction context at startup. Where supported, GAL dispatch adapters pass flags to suppress project-level instruction injection so that the self-contained task spec governs execution without interference:

| Executor | Exposure Status | Suppression Mechanism | Verified Version | Isolation Behavior |
| --- | --- | --- | --- | --- |
| **codex** | Exposed (`AGENTS.md`) | `-c project_doc_max_bytes=0` | codex-cli 0.149.1 | Unconditionally passes `-c project_doc_max_bytes=0` to suppress repository-level `AGENTS.md`. The global `~/.codex/AGENTS.md` layer remains active. |
| **copilot** | Exposed | `--no-custom-instructions`, `--disable-builtin-mcps` | Copilot CLI | Appends flags to disable custom instructions and built-in MCPs, ensuring prompt-mode self-containment. |
| **claude** | Exposed (`CLAUDE.md`) | `--setting-sources user` | Claude Code 2.1.251 | Unconditionally passes `--setting-sources user` to suppress repository-level `CLAUDE.md` and project settings while retaining user configuration. |
| **opencode** | `NotRun` | None | opencode 1.18.25 | Exposure probe encountered provider-side timeouts during testing; no adapter suppression is applied. Treated as inconclusive rather than unexposed. |
| **agy** | Exposed by design | None (Task-spec boundary precedence) | agy 1.1.23 | Passes `--add-dir` deliberately to confine workspace access and supply repository context; task-spec `## Dispatched Worker Boundary` takes precedence in model instruction context. |

## Git Helpers

### `gal commit-msg`

The `gal commit-msg` command operates as the deterministic commit helper supporting the `git-commits` skill and the `git-commit-msg` command. The type and scope classification stems **exclusively** from changed file paths and git status. It ignores diff and body keywords, preventing message content from hijacking classifications. Three modes exist:

- **`gal commit-msg --context`** outputs a compact staged-changes context containing files, deterministic type and scope baseline headers, staged plan and prompt summaries, and hunk headers targeted at message-drafting agents. This mode delivers higher signal quality than raw diffs at reduced token costs.
- **`gal commit-msg --print`** outputs exclusively the deterministic type and scope subject header.
- **`gal commit-msg <file>`** acts as the git commit-msg hook. It populates blank messages based on staged changes and strictly avoids overwriting authored content.

The `git-commit-msg` command generates message wording exclusively and avoids executing git commit. The `git-commits` skill generates the message **and** executes the commit upon detecting explicit commit intent.

### Git Filters (`gal clean` / `gal smudge`)

The optional `gal-config` git filter eliminates machine-local configuration values from tracked files. Apply per-repository registration:

```bash
git config filter.gal-config.clean 'gal clean'
git config filter.gal-config.smudge 'gal smudge'
```

The git executable invokes this filter internally. The `gal` binary requires presence on the active git `PATH` to prevent `git commit` failures resulting from filter errors.

## Authoring Helpers

### `text-flowcharts`

The [`text-flowcharts`](../plugins/gal-core/skills/text-flowcharts/SKILL.md) skill renders branching logic, pipelines, and multi-step processes as monospaced text decision-tree diagrams. Invoke it with `/text-flowcharts` in Claude Code, or `$text-flowcharts` in Codex. It also activates automatically whenever an explanation traces per-record control flow, or when you ask for a flowchart, 流程圖, or 邏輯圖.

Each diagram follows one record from entry at the top down through the conditions it meets to a graded terminal outcome, so the reader can drop a single item in and watch where it lands. The vocabulary stays deliberately small. Boxes hold steps, braces mark decisions, and every leaf carries an end-state marker.

| Element | Glyph |
| --- | --- |
| Flow lines and corners | `│ ─ ┌ ┐ └ ┘` |
| Junctions (split, merge, cross) | `├ ┤ ┬ ┴ ┼` |
| Arrowheads (down, up, right, left) | `▼ ▲ ▶ ◀` |
| Terminal success | `√` |
| Skipped on purpose | `>>\|` |
| Dead end or rejected | `×` |

These glyphs render in default monospace fonts without a font install, sit in a single cell for non-CJK readers, and survive in pull request comments, code comments, and terminals under UTF-8. The output carries no emoji, so it stays legible on older machines. Use the skill only when a process actually branches. A straight sequence with no decisions reads better as a numbered list.

## Golem Agents

GAL specialist agents act as golems. This section provides the **user-facing** perspective regarding functionality, differentiation, and application. The authoritative roster and classifications reside in [`plugins/gal-core/agents/agents.md`](../plugins/gal-core/agents/agents.md). Workflow semantics reside in [`plugins/gal-core/workflows/coding.md`](../plugins/gal-core/workflows/coding.md).

### Capability Table

| Golem | What it does | When to use it |
| --- | --- | --- |
| `golem-architect` | Adversarial plan review covering trade-offs, over-engineering, bug surface, and dependency/API risk | Utilize `/deep-planning` or `/gal architect` to pressure-test designs prior to building |
| `golem-analyst` | Business-logic review covering ROI, domain correctness, and user impact | Utilize when changes affect pricing, permissions, eligibility, or customer-visible rules |
| `golem-designer` | UI/UX experience design, DevEx, design systems, accessibility, and live UI audits | Utilize for customer-facing layout, state, component work, or developer-facing DevEx |
| `golem-researcher` | Local-first investigation, cross-source synthesis, and reference-ready findings | Utilize `/gal research` or `/gal deep-research` for evidence-backed answers |
| `golem-implementer` | Writes implementation code for approved tasks via atomic commits | Represents the CODER phase within `/gal pipeline` |
| `golem-tester` | Generates spec-driven tests and real-browser QA derived strictly from the plan spec | Represents the TESTER phase driving independent verification utilizing a distinct model |
| `golem-auditor` | Executes deep performance and security audits of a single task | Represents the AUDITOR phase in `/gal pipeline` (task audit). This role remains orchestrated-only, avoiding bare `/gal auditor` calls |
| `golem-debugger` | Conducts scientific-method bug investigations employing freeze discipline and root-cause confirmation | Utilize when bugs demand disciplined investigation prior to fixes |
| `golem-steward` | Manages documentation structure, code-doc drift, knowledge extraction, and figure synchronization | Utilize `/gal steward` or trigger automatically at planning-open, refining-end, or pipeline-closeout |
| `golem-releaser` | Designs planning-stage release flows via API and CICD research to emit design advice | Utilize `/gal releaser` (isolated) or `/gal discuss releaser` (in-context) prior to `/planning release-<slug>` |

**Checking-role triangle:** Quality assurance responsibilities form a tripartite structure. The **ORCHESTRATOR** (pipeline) commands per-task correctness gating, end-of-run goal-backward verification, and plan lifecycle closure. The **AUDITOR** manages single-task deep performance and security. The **STEWARD** controls documentation structure.

### Role Invocability Matrix

| Role | Directly callable? | Mode(s) |
| --- | --- | --- |
| **architect** | Yes | `/gal architect` (isolated) or `/gal discuss architect` (in-context) |
| **analyst** | Yes | `/gal analyst` (isolated) or `/gal discuss analyst` (in-context) |
| **designer** | Yes | `/gal designer` (isolated) or `/gal discuss designer` (in-context) |
| **releaser** | Yes | `/gal releaser` (isolated) or `/gal discuss releaser` (in-context), planning designer, does not execute |
| **debugger** | Yes | `/gal debugger` |
| **steward** | Yes | `/gal steward` |
| **implementer** | **Orchestrated-only** | Exclusively via `/gal pipeline` (pipeline-phase context) |
| **tester** | **Orchestrated-only** | Exclusively via `/gal pipeline` (pipeline-phase context) |
| **auditor** | **Orchestrated-only** | Via `/gal pipeline` (pipeline-phase). The auditor has no whole-branch mode. The `--finalize-branch-audit` token is still recognised and still returns a `COMMAND: error` non-dispatch block: `/gal finalize` runs its own top-down review in the same runtime, marks `Review Independence: DEGRADED_SAME_RUNTIME`, and writes the Requirement-by-L1-L4 table. |
| **researcher** | **Orchestrated-only** | Exclusively via `/gal research` or `/gal deep-research` |

Executing `/gal <role>` lacking the matching orchestration context yields `COMMAND: error` for all four orchestrated-only roles.

### Consult Dual-Mode (`/gal discuss <role>`)

The architect, analyst, designer, and releaser roles support two invocation modes exclusively. Remaining roles do not support the discuss format.

| Mode | Trigger | What happens | Response label |
| --- | --- | --- | --- |
| **Isolated** (default) | `/gal <role>` | Native subagent runs the role in isolation. Only verdict and summary return to main context | `[<role> · isolated]` |
| **In-context** | `/gal discuss <role>` | Role activation core loads into the current conversation. Assistant hot-joins from prior isolated verdicts and continues multi-turn until topic changes | `[<role> · in-context]` |

**Hot-join:** The transcript contains the isolated verdict, enabling in-context mode to resume progress natively bypassing complete role re-execution.

The Codex runtime maps `/gal discuss <role>` to `$discuss-<role>`. The Claude runtime implements `/gal discuss <role>` directly as a slash command.

The integrated [`adversarial-review`](../plugins/gal-core/skills/adversarial-review/SKILL.md) skill provides the target-agnostic review methodology utilized by these review roles. It enforces steel-man argumentation, refute-by-default logic, evidence discipline, and explicit APPROVE, REVISE, or REJECT verdicts.

### Steward Lifecycle Split

The steward executes during planning and finalize phases, performing distinct operations. These phases remain strictly non-interchangeable:

- **Planning Phase (`/deep-planning`): Plan-document structure focus.** The steward evaluates the plan file attributes, including path, slug, mandatory sections, language consistency, and diagram synchronization. It prohibits writing to `docs/` during this phase. Planning-stage documents lack implementation and durable knowledge. Writing to `docs/` risks polluting reader-facing layers with unbuilt, speculative content.
- **Finalize Phase (`/gal finalize`): Durable knowledge landing.** Completed plan durable knowledge mandates landing within `docs/` as a strict rule. The steward extracts implemented plan knowledge into the durable layer (`README.md` and `docs/`). Subsequently, the index synchronizes into `.dev/project.md`. This landing process is enforced rather than advisory. Plan file deletion requires the steward to generate the durable-layer commit first.

This structural split recognizes that knowledge extraction necessitates built knowledge accessible exclusively post-implementation. Consequently, writing finished plans into proper documentation remains a finalize-stage operation. The planning-time steward strictly focuses on maintaining plan document formatting.

### Release-Plan Lane

Releases constitute distinct plan types (`release-<slug>`) and operate independently of `/gal pipeline` phases and `/gal finalize` steps.

**Flow:**

1. The `/gal releaser` command verifies the deploy target prior to resolving locally-available deploy capabilities from read-only sources including skills, APIs, and CLI config files. It subsequently designs the release and devops flow. The operation is strictly read-only, avoiding file writing, committing, or execution. Unidentifiable capabilities yield not-available reports, preventing fabrication.
2. The `/planning release-<slug>` command materializes the generated advice into a source plan featuring a `## Tasks` section.
3. The standard `/gal pipeline` command executes the finalized release plan.
4. The `/gal finalize` command lands the release plan identical to standard plans.

This process supports GAL-self CLI binary releases alongside downstream repository deployments (e.g., web services, backend services, firebase-class, npm packages, Docker images). The operational scope halts at design advice. The releaser prohibits acting as a deploy orchestrator and omits canary, rollback, and production monitoring features.
