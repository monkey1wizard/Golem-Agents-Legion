# GAL User Manual

**English** · [日本語](i18n/ja/manual.ja.md) · [繁體中文](i18n/zh-Hant/manual.zh-Hant.md)

The GAL daily usage guide covers installation, initial repository setup, workflow operations, configuration, personalization, headless executor routing, and golem agents.

This manual details **GAL operation**. System architecture (codebase and `~/.gal/` topology, release lineage) is documented in [`docs/architecture.md`](architecture.md). Maintainer procedures (distribution mechanics, provider packaging, modification guidelines) are documented in the [developer guide](devguide.md). These topics are excluded from this manual.

## Overview

GAL uses `~/.gal/plugins/gal/` as the canonical plugin root. Provider-visible targets function as projections rather than content owners. Configuration is stored in `~/.gal/config/config.json`. Refer to [architecture → `~/.gal/` runtime layout](architecture.md#gal-runtime-layout) for `~/.gal/` layout and ownership boundaries.

The `gal` binary automatically locates its source root through a `.git`-bounded current working directory search followed by a binary-side packaged layout. The `devMode` and `galRoot` configuration keys are neither required nor read.

## Installing GAL

Select the appropriate platform installation option. After installation, execute `gal init` within the repository to generate repo-local adapters (`CLAUDE.md`, `AGENTS.md`, etc.) — refer to [First Run in Your Repo](#first-run-in-your-repo).

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

#### From the Claude / Codex marketplace (chat-driven binary install)

Alternatively, discover GAL as a plugin and authorize the AI to complete the setup:

1. Locate and install the **GAL plugin** within the Claude Code or Codex plugin marketplace (search "gal"), or add it from the [marketplace snapshot branch](https://github.com/monkey1wizard/golem-agents-legion/tree/marketplace-snapshot).
2. Request installation via **"help me install gal"**. The plugin provides an `install-gal` skill. Upon consent, it executes the OS-appropriate option (Homebrew / winget / `cargo install --git`), verifies the outcome, and routes the process through `gal init`. Unavailability of options is reported directly.

The standalone plugin does not constitute a functional GAL installation. All commands excluding `install-gal` require the `gal` binary, making step 2 mandatory for installation completion. The `install-gal` skill excludes binary bundling, requires command preview prior to execution, and strictly reports factual success states.

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

The `gal init` command generates five repo-local adapter roots (`CLAUDE.md`, `AGENTS.md`, `GEMINI.md`, `.github/copilot-instructions.md`, `.agents/rules/gal.md`) from `gal-core` templates and scaffolds the `.dev/` directory during initial execution. Executing with the `--force` flag regenerates adapters idempotently.

Executions output one row per touched adapter path, classified as `Written` (created or modified content), `Unchanged` (byte-identical, write bypassed), or `Removed` (unselected stale GAL-owned conditional layer).

> ⚠️ The `--force` flag initiates bootstrap-overwrite. This action replaces `.dev/project.md` completely with the template and discards existing repository project content. Avoid using this flag to bypass initialization errors. Resolve the reported issue directly (refer below).

### `.dev/project.md` in your Project

The `.dev/project.md` file serves as the compressed project summary utilized for adapter rendering. Eight H2 sections are **strictly required once each**: `What This Is`, `Tech Stack`, `Architecture`, `Constraints`, `Response Style`, `Freshness`, `Project Language`, `Protected Paths`.

This requirement operates as fail-closed. Missing or duplicated sections cause `gal init` to reject the entire render, identify the problematic heading, and halt adapter file writing. Errors report one missing heading per execution. Files missing multiple sections require iterative correction passes. Add the identified section manually (referencing `plugins/gal-core/templates/project.md` format), re-execute, and append subsequent sections.

The `.dev/project.md` file enforces a strict size limit. Rejections based on size require content trimming rather than execution retries. The rendering process strictly avoids writing partial adapter sets to bypass size constraints.

### `gal doctor`

The `gal doctor` command verifies local setup health, encompassing the binary, canonical root, runtime surfaces, and configuration. Execute this command after installation, after upgrading, or upon agent failure to detect GAL commands.

Within initialized repositories, the command additionally outputs a read-only adapter-size advisory table covering the five adapter roots. Oversized adapters generate a `[WARNING]` finding rather than an error, preventing isolated run failures.

Regarding the opt-in self-test for headless coding agent execution, refer to [Executor Self-Test](#executor-self-test-gal-doctor---executor-smoke).

### Triggering GAL Commands per Runtime

GAL commands initialize via runtime-specific mechanisms, resulting in distinct triggers:

| Runtime | Trigger | Notes |
| --- | --- | --- |
| Claude Code | `/gal status` | native plugin command |
| Codex | `$gal-status` | exposed as a skill (`$` prefix, or `/skills`) |
| Copilot | `/gal-status` | exposed as a skill (`/skills list` to browse) |
| Antigravity | `/gal-status` | exposed as a skill (Antigravity lacks native `commands/` folders, commands are Agent Skills) |
| Gemini | `/gal-status` | native TOML command (hyphen, not space) |

Note the syntax difference. Non-Claude runtimes utilize `gal-status` (hyphen) instead of `gal status` (space).

### When Command Changes Appear

Command update visibility depends on runtime loading mechanics:

- **Claude Code** caches plugins. **Restart Claude Code** to load updated commands.
- **Antigravity** registers commands during startup. **Restart agy** to load changes.
- **Gemini / Copilot / OpenCode** read command and skill files directly during each new session.
- **Codex** auto-detects skill changes within the active thread per documented primary behavior. **Restart Codex or initialize a new thread** as a fallback if changes fail to appear.

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
- **Execution phase:** The pipeline iterates tasks autonomously following an implement, test, audit, and commit sequence per task. User intervention remains unnecessary barring workflow interruption.
- **Interruption conditions:** The pipeline halts exclusively for human-decision blockers, tasks reaching the retry ceiling (three failed verification attempts), or configured working-hours hard stops. Upon interruption, the execution prompt logs an interrupted-phase note detailing the stoppage point and pending requirements.
- **Resume protocol requires re-execution.** Resolve the blocker or underlying cause, then re-execute the identical `/gal pipeline` command. Execution resumes from the recorded cursor. Completed tasks bypass re-execution.

### Finalize: What Lands and What Gets Deleted

The `/gal finalize` command orchestrates the closure of a completed plan. Preconditions enforce exhaustive task completion and verification, proven via a zero-trust machine receipt (`gal finalize-check`).

- **Landed components:** A holistic cross-task branch review precedes doc-sync. The STEWARD agent must extract the plan's durable knowledge into `README.md` and `docs/`. A merge to main follows, including worktree teardown if applicable.
- **One conflict shape resolves itself.** When the merge to main conflicts and the unmerged path set is exactly `{.dev/state.md}`, finalize runs the internal `gal state-merge` resolver, which merges the plan-keyed tables row by row and stages the result. Exit 0 continues the landing. `STATE_MERGE: unresolved` stops with the repository proven byte-identical to before the attempt, and `STATE_MERGE: rollback-unconfirmed` stops and names `.dev/state.md` as possibly mutated, requiring inspection. Every other conflict shape keeps the unconditional stop. Mechanism and rejected alternatives: `docs/architecture.md`.
- **Deleted components:** Plan files within `.dev/plans/` undergo removal only after documentation is committed and a post-write hygiene check passes. This ensures knowledge transfer to the durable layer prior to file erasure.
- **Retained components:** A close-out row is written to `.dev/state.md` containing the date, plan, and landing commit. The `gal-last-good` tag is assigned to the landing commit.

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

Machine-local values reside in `~/.gal/config/config.json`. Credentials map to `secrets`, retained machine paths to `galSkills`, and working-hours to `workingHours`. Additional keys include `planLanguage`, `memoryHarvest`, and `executorRouting` (detailed in [Headless Executors](#headless-executors)). Strictly prohibit writing local values into tracked documentation, command templates, or source files.

| Placeholder | Meaning | Common use |
| --- | --- | --- |
| `<WORKING_HOURS_ENABLED>` | whether working-hours enforcement is active | opt-in wrap-up and hard-stop enforcement |
| `<WORKDAY_START>` / `<WORKDAY_END>` | preferred workday in `HH:MM` | Working Hours schedule / After Hours boundary |
| `<WRAP_UP_TIME>` / `<HARD_STOP_TIME>` | wrap-up and hard-stop in `HH:MM` | shutdown-window / stop-work behavior |
| `<GAL_SKILLS>` | absolute path to the machine-local GAL skills directory | git filters and machine-local skill projections |
| `<CONTEXT7_API_KEY>` | Context7 API key for runtimes that require it | MCP merge (materialized into `~/.gal/generated/mcp/managed.json`) |

### Secrets & MCP Overrides

The tracked GAL source in `plugins/gal-core/mcp.json` and bundle manifests form the singular source for MCP servers. Machine-specific variations utilize `${ENV_VAR}` placeholders resolved during projection via `~/.gal/config/config.json` from its `secrets` map and machine paths. Separate override files are unsupported.

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

Define matching values within the `secrets` map in `~/.gal/config/config.json`. The `${ENV_VAR}` placeholders in MCP configurations resolve during runtime execution. Installed runtime MCP configurations retain user ownership. Prevent committing storage-state files, persistent profiles, browser artifacts, and secret-like local files.

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

**Source 1 — personal convention file (always-on, per-repo).** Duplicate the included example from `plugins/gal-core/templates/csharp-convention.example.md` to `~/.gal/local/conventions/csharp.md`, strictly matching this destination path, and modify the content to reflect the target style. Execute `gal init` within any target repository featuring a matching `Language` row in `.dev/project.md`. The system injects the file into the repository adapters mirroring the gal-core convention process.

**Source 2 — installed agent-plugin detection (read-only, zero setup).** Existing official language plugins installed within the coding agent require only a `gal init` execution in the target repository. GAL correlates the plugin skill name and origin against the repository `Language` row and renders a **Detected Language Skills** reference block containing the name, origin path, and a persistent load instruction. GAL strictly avoids copying skill content and abstains from installing, updating, or removing plugins. Newly installed plugins require a subsequent `gal init` execution for detection.

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

The `config.json#executorRouting` key categorizes roles **by consumer** into two distinct objects. The `pipeline` object handles headless dispatch of implement, test, and audit phases. The `planning` object manages Codex native-subagent model selection for planning-stage review roles and strictly avoids headless dispatch. A shared `executors` default-model block accompanies these objects:

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
    }
  }
}
```

Both groups enforce closed role allowlists. The `pipeline` list includes `{CODER, TESTER, AUDITOR}`. The `planning` list includes `{ARCHITECT, ANALYST, DESIGNER, RELEASER}`. Roles assigned to incorrect groups or unrecognized roles trigger skipping and generate warnings specifying the correct group. The legacy flat structure featuring role keys directly beneath `executorRouting` is retired, producing visible warnings without fallback mechanisms.

**executors block:** Defines the default model per tool. Role entries lacking a model specification inherit the default from `executors[executor]`. Explicit model declarations on a role supersede defaults.

**Per-role effort key:** Provides an optional reasoning-intensity indicator which maps to the native reasoning flag of each executor prior to spawn:

| Executor | `effort` → native flag |
| --- | --- |
| claude | `--effort <value>` |
| codex | `-c model_reasoning_effort="<value>"` |
| copilot | `--reasoning-effort <value>` |
| opencode | `--variant <value>` |
| agy | unsupported |

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
| `CODER` | pipeline | Writes implementation code following a plan. Must differ from `TESTER` |
| `TESTER` | pipeline | Writes tests from plan spec and public API only. Must differ from `CODER` |
| `AUDITOR` | pipeline | Audits deep performance and security. Must differ from `CODER`, tier >= `CODER` |
| `ARCHITECT` | planning | Adversarial plan review covering trade-offs, over-engineering, and bugs |
| `ANALYST` | planning | Business logic review covering ROI, domain correctness, and user impact |
| `DESIGNER` | planning | UX, UI, and DevEx review |
| `RELEASER` | planning | Release-flow design (read-only) |

Research operations execute **in-process and resist headless routing**. The `RESEARCHER` role lacks an `executorRouting` entry. The `/gal research` and `/gal deep-research` commands execute within the conversation loop. Valid executors include `claude`, `codex`, `opencode`, `copilot`, and `agy`. Omitting a role retains its execution within the conversation loop.

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
| `golem-auditor` | Executes deep performance and security audits of single tasks and whole branches | Represents the AUDITOR phase in `/gal pipeline` (task audit) or `/gal finalize` (whole-branch audit). This role remains orchestrated-only, avoiding bare `/gal auditor` calls |
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
| **auditor** | **Orchestrated-only** | Via `/gal pipeline` (pipeline-phase) or `/gal finalize` (branch-audit) |
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

1. The `/gal releaser` command verifies the deploy target prior to resolving locally-available deploy capabilities from read-only sources including MCP configs, skills, APIs, and CLI config files. It subsequently designs the release and devops flow. The operation is strictly read-only, avoiding file writing, committing, or execution. Unidentifiable capabilities yield not-available reports, preventing fabrication.
2. The `/planning release-<slug>` command materializes the generated advice into a source plan featuring a `## Tasks` section.
3. The standard `/gal pipeline` command executes the finalized release plan.
4. The `/gal finalize` command lands the release plan identical to standard plans.

This process supports GAL-self CLI binary releases alongside downstream repository deployments (e.g., web services, backend services, firebase-class, npm packages, Docker images). The operational scope halts at design advice. The releaser prohibits acting as a deploy orchestrator and omits canary, rollback, and production monitoring features.
