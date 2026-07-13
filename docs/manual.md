# GAL User Manual

Everything for running gal on your machine: runtime selection, configuration and placeholders, model routing, MCP overrides, local-notes routing, working hours, and headless executor routing.

This manual owns **how you operate GAL**. The architecture behind it (codebase + `~/.gal/` structure, distribution, release lineage, provider packaging) lives once in the [developer guide](devguide.md) and is not repeated here.

## Overview

GAL uses `~/.gal/plugins/gal/` as the canonical plugin root; provider-visible targets are projections, not content owners. Configuration lives in `~/.gal/config/config.json`. For the `~/.gal/` layout and ownership boundaries see [developer guide → .gal Data Structure](devguide.md#gal-data-structure).

The `gal` binary locates its source root automatically via a `.git`-bounded cwd-walk then a binary-side packaged layout. No `devMode` or `galRoot` config keys are required or read.

## First-Time Setup

### Installing GAL

Choose the channel that fits your platform. After any install, run `gal init` in your repo to generate the repo-local adapters (CLAUDE.md, AGENTS.md, etc.).

#### `cargo install --git` (from source — available now)

```bash
cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli
```

GAL is not on crates.io — always use `--git`. The `gal-cli` package name is required in a Cargo workspace; the installed binary is named `gal`.

#### Homebrew (macOS / Linux — recommended once live)

```bash
brew install monkey1wizard/tap/gal
```

_Future milestone — not yet live; Homebrew tap pending first GitHub Release._

#### curl (Linux / macOS — requires a published GitHub Release)

```bash
curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | bash
```

Verifies SHA-256 against `checksums.txt` (mandatory; aborts on mismatch) and optionally the cosign keyless signature. Installs to `~/.local/bin`. Override the version with `GAL_VERSION`.

#### irm (Windows — requires a published GitHub Release)

```powershell
# Windows (BETA — not yet validated on a clean machine)
irm https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.ps1 | iex
```

Same SHA-256 verification. Override the version with `$env:GAL_VERSION`.

#### winget (Windows — future milestone)

```
winget install Monkey1Wizard.GAL
```

_Future milestone — pending first GitHub Release and winget-pkgs submission._ For v1, manifests in `packaging/winget/` are submitted manually as a PR to [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs/tree/master/manifests/m/Monkey1Wizard/GAL/).

#### Prebuilt binary (manual install — requires a published GitHub Release)

Download `gal-<version>-<platform>-<arch>[.zip|.tar.gz]` from the GitHub Releases page, extract, and place `gal` (or `gal.exe`) on your `PATH`.

#### From the Claude / Codex marketplace (chat-driven binary install)

Instead of installing the binary first, you can discover GAL as a plugin and let your AI finish setup:

1. Find and install the **GAL plugin** in the Claude Code or Codex plugin marketplace (search "gal"), or add it from the [marketplace snapshot branch](https://github.com/monkey1wizard/golem-agents-legion/tree/marketplace-snapshot).
2. Say **"help me install gal"**. The plugin ships an `install-gal` skill: with your consent it runs the OS-appropriate channel above (Homebrew / winget / `cargo install --git`), then verifies `gal --version` and reports honestly if no channel is available.

The plugin alone is **not yet a working GAL** — every command except `install-gal` needs the `gal` binary, so step 2 completes the install. The `install-gal` skill never bundles a binary, never runs anything without showing the exact command first, and never reports a success that did not happen.

---

`gal init` generates the repo-local adapters (CLAUDE.md, AGENTS.md, GEMINI.md, .github/copilot-instructions.md) from the gal-core templates. Re-running it with `--force` regenerates them idempotently.

### Bounded Project Index

`.dev/project.md` is a **bounded current-topic index**, not a growing changelog. Its content shape and size budget are owned by `plugins/gal-core/conventions/token-budget.md` § Bounded Current-Topic Index: `## Verified Facts` holds one bullet per fixed topic area (runtime/layout, lifecycle/control plane, adapters, documentation/structure map, planning/review/gates, dispatch/remote, naming/personalization/Core, install/release/restore, Codex compatibility, research), each stating only the topic's **current** state with a durable pointer into `README.md` or `docs/`. Updating a topic is **upsert/replace/prune** — never append a new bullet to record that a plan finished. `## Suspected Drift` and `## Documentation Gaps` read exactly `None.` when nothing is outstanding.

**Write-boundary validation.** The repo-adapter render enforces a hard cap: `.dev/project.md`, normalized to LF line endings, must not exceed 30,720 bytes UTF-8. The check runs immediately after reading `.dev/project.md` — before any of the five adapters are written — so an oversized index fails the render with **no partial output** (none of the five adapters are touched). If `gal init` or `gal refresh` rejects your index on size, trim the offending topic bullet rather than retrying; the render never writes a partial adapter set to work around the cap.

**Three skill-index carriers vs. two that omit it.** `CLAUDE.md`, `AGENTS.md`, and `GEMINI.md` place the complete `## Repo Skills` section **before** the `.dev/project.md` content in the rendered adapter (so the skill index is visible even on a truncated read). `.github/copilot-instructions.md` and `.agents/rules/gal.md` omit the `## Repo Skills` section entirely — Copilot discovers GAL commands through the global runtime install, and `.agents/rules/gal.md` is a plain workspace-rule carrier with no skill-discovery role.

**Post-doc-sync rendering.** At `/gal finalize`, STEWARD's `.dev/project.md` reindex (extracting durable knowledge into `docs/` first, then re-syncing the index per the rule above) runs **before** any adapter-render idempotency evidence is collected. A double render performed before the reindex reflects the pre-doc-sync content and cannot stand in as post-doc-sync evidence — the fresh double render, over all five adapters, must run after the reindex lands. See [Internal Binary Subcommands](#internal-binary-subcommands) and `plugins/gal-core/commands/gal-finalize/SKILL.template.md` Sequence 2.

### Runtime Selection

gal projects to all 5 runtimes (Copilot, Codex, Antigravity, Gemini, OpenCode) by default. There is no separate installer step and no persisted selection file — `gal refresh` renders the canonical root and projects every runtime surface. Selective per-runtime projection is a future enhancement.

The GAL repo remains the single source of truth for `plugins/gal-core/agents/`, `plugins/gal-core/skills/`, and `plugins/gal-core/commands/`. Antigravity CLI (AGY) is the primary Google terminal runtime; gal links each AGY surface to the canonical root at `~/.gal/plugins/gal/`.

The gal command surface is split by concern: `gal init` generates repo-local adapters, `gal update` prints the current version and upgrade instructions, and `gal doctor` checks the local setup. Binary installation and upgrades use your package manager (cargo/winget/Homebrew/curl). See [Machine Operations](#machine-operations).

#### How to trigger GAL commands per runtime

GAL commands load through each runtime's own mechanism, so the trigger differs:

| Runtime | Trigger | Notes |
| --- | --- | --- |
| Claude Code | `/gal status` | native plugin command |
| Gemini | `/gal-status` | native TOML command (hyphen, not space) |
| Copilot | `/gal-status` | exposed as a skill (`/skills list` to browse) |
| Codex | `$gal-status` | exposed as a skill (`$` prefix; or `/skills`) |
| Antigravity | `/gal-status` | exposed as a skill at `~/.gemini/antigravity-cli/skills/` (Antigravity has no native `commands/` folder; commands are Agent Skills) |

Note the spelling: every non-Claude runtime uses `gal-status` (hyphen), not `gal status` (space). The developer-facing projection matrix is in `docs/devguide.md` (Per-Runtime Command Loading).

#### When new/updated commands appear

How a changed command becomes visible depends on how the runtime loads it:

- **Claude Code** caches its plugin under `~/.claude/plugins/cache/gal/`. **Restart Claude Code** to pick up updated commands. (No manual `/plugin marketplace update` needed.)
- **Antigravity** registers commands at startup — **restart `agy`** to pick up changes.
- **Gemini / Copilot / OpenCode** read their projected command/skill files directly on each new session.
- **Codex** auto-detects skill changes in the running thread as its documented primary behavior; **restart Codex or open a fresh thread** is the fallback when a change does not appear. Codex does *not* require a brand-new shell session as a rule — see [Codex Skill Surface Troubleshooting](#codex-skill-surface-troubleshooting) below for the full three-state picture.

Fully automatic refresh-on-commit (Claude git-source marketplace) is deferred until GAL ships a public GitHub release.

#### Codex Skill Surface Troubleshooting

`~/.agents/skills` is Codex's official user-skill root — and it is **shared with other tooling** (e.g. `vercel-labs/skills`-family CLIs also manage entries there via their own lockfile). GAL never places a symlink/junction in that root; every GAL entry is a **materialized real-directory copy** (the same mechanism command-skills already used, now also used for core skills), so an external tool clearing junctions there cannot wipe GAL's projection in one shot — a deleted GAL skill degrades to stale-but-recoverable (`gal refresh` restores it), not gone.

Three things to know when a GAL skill does not appear as expected:

1. **Auto-detect vs. restart.** Codex's documented primary behavior is automatic detection of skill changes within the current thread. If a change genuinely does not appear, restart Codex or open a fresh thread, then verify with `/skills` or by invoking `$gal-status` directly.
2. **Context-budget omission is not a failure.** Codex caps the initial skill list at roughly 2% of the context window (an 8,000-character fallback when that budget is unknown). When the total exceeds the cap, Codex first shortens descriptions and then omits skills from the initial list — an omitted skill is still directly callable via `$skill-name`. `gal doctor` / `gal refresh` warn when GAL's own projected description footprint nears this cap.
3. **Same-name dual-listing.** If more than one source (e.g. GAL and another skill-managing tool) projects a skill with the same name, Codex does not merge them — both can appear in the skill selector.

`gal doctor` diagnoses the shared-skills surface directly:

- **Missing-skill discriminator** — a required GAL skill absent from disk is reported as either `external deletion (last GAL projection: <timestamp>)` (still listed in GAL's own projection lockfile) or `not projected / pruned` (never was); when no lockfile is available the cause is reported as `unknown`, never guessed.
- **Zombie detection** — a shared-root entry GAL owns but no longer has an active command/skill counterpart for (e.g. a retired command) is flagged and cleaned by `gal refresh`.
- **Config-disable scan** — a global `~/.codex/config.toml` `[[skills.config]]` entry that disables a GAL skill (`enabled = false`) is flagged. Project-local and sub-agent `[[skills.config]]` variants are documented upstream but do not currently take effect, so only the global file is scanned.

Fix path for all of the above: `gal refresh`.

#### Codex GAL Pipeline Troubleshooting

When `$gal-pipeline` in Codex silently degrades into generic chat work instead of running the Rust dispatch path, walk this checklist — each item maps to a durable `gal doctor` finding or a documented behavior:

1. **Is the loaded skill current?** After a source-contract change you must run `gal refresh` (then restart Codex / open a fresh thread). `gal doctor` reports a **stale skill projection** when a projected `~/.agents/skills/<name>/SKILL.md` differs from current canonical content.
2. **Is the binary current?** In a GAL checkout, `gal doctor` reports **binary/source skew** when the `gal` binary on PATH was built from an older commit than the checkout HEAD — rebuild + reinstall (`cargo build --release -p gal-cli`) before trusting projection/dispatch output. This is distinct from a stale skill.
3. **Quote `#file:` in PowerShell.** A bare `#file:<prompt>` is a **comment** in PowerShell, so the prompt path is stripped and `gal dispatch-script` emits `COMMAND: error`. Always paste `'#file:.dev/plans/<slug>.prompt.md'` single-quoted.
4. **Sandbox write denial is a hard stop.** If a receipt/log write under `.dev/pipeline/receipts/` or `.dev/executor-logs/` is denied (Codex sandbox) — or a dispatch reports `reason=log-error` — **stop and rerun the exact Rust command with write approval**. Never role-play the phase in chat because a write was denied.
5. **Generic pipeline block ≠ phase execution.** `gal dispatch-script pipeline` only loads the `/gal-pipeline` procedure; phase execution begins only after the preflight receipt passes and each phase is dispatched via `gal dispatch-script … '#file:<prompt>'`.
6. **Multiple active plans → name the prompt.** With more than one active plan, pass an explicit `'#file:<prompt>'`; never let selection fall through to the first plan row.
7. **Adapter prompt budget.** The Named Workflow Obedience bullet + the compact GAL critical runtime pack render into AGENTS.md within Codex's `project_doc_max_bytes` budget, so a partial project-doc read still sees the contract.
8. **Skill-budget pressure.** `gal doctor` names the largest per-skill description contributors when the projected footprint nears Codex's initial-list budget; an omitted skill is still callable via `$skill-name`.
9. **Codex Memories are advisory, not authority.** `gal doctor` flags **enabled** Codex Memories as an advisory: they are Codex's own store, not GAL file memory. GAL workflow authority is always the repo `.dev/` files. A **disabled** memory is never the cause of GAL drift — do not enable Memories to "fix" a workflow problem.
10. **Configured MCP ≠ exposed tools.** A configured `~/.codex/config.toml` `[mcp_servers.*]` server (also surfaced advisory by `gal doctor`) does not mean its graph/tools are exposed in the session; structural-retrieval guidance capability-gates on the tools actually available and falls back to repo-native discovery otherwise.
11. **Nested Codex readiness.** If `codex` is a routed executor but its `codex exec` probe reports an arg0-cleanup / PATH-alias **access-denied** warning, `gal doctor`/dispatch marks readiness DEGRADED — ensure the temp dir is writable and PATH resolves the real `codex` binary.

### Configuration & Placeholders

Machine-local values live in `~/.gal/config/config.json` — credentials under `secrets`, retained machine paths under `galSkills`, working-hours under `workingHours`, and `planLanguage`. Never write local values into tracked docs, command templates, or source files.

| Placeholder | Meaning | Common use |
| --- | --- | --- |
| `<WORKING_HOURS_ENABLED>` | whether working-hours enforcement is active | opt-in wrap-up and hard-stop enforcement |
| `<WORKDAY_START>` / `<WORKDAY_END>` | preferred workday in `HH:MM` | Working Hours schedule / After Hours boundary |
| `<WRAP_UP_TIME>` / `<HARD_STOP_TIME>` | wrap-up and hard-stop in `HH:MM` | shutdown-window / stop-work behavior |
| `<GAL_SKILLS>` | absolute path to the machine-local GAL skills directory | git filters and machine-local skill projections |
| `<CONTEXT7_API_KEY>` | Context7 API key for runtimes that require it | MCP merge (materialized into `~/.gal/generated/mcp/managed.json`) |

### Companion Plugins

Third-party plugin/skill catalogs and cross-agent installation are handled by the separate **ccync** product, not by gal. gal projects only its own plugin (plus the hand-placed personal layer described below).

## Personalization

### Personal Content Layer (~/.gal/local/)

Machine-local personal content lives under `~/.gal/local/` and is projected to all agents (Claude, Copilot, Codex, AGY) via canonical-root render. gal reads `local/skills/`, `local/mcp.json`, and `local/conventions/` but **never writes or deletes those user-authored paths**. Cross-machine sync is not handled by gal (local-only this round). (Managing third-party *plugins* — a catalog of installable plugin units — is the ccync product, not gal.)

**Layout:**

```
~/.gal/local/
  skills/
    <skill-name>/
      SKILL.md          ← hand-placed personal skill (gal read-only)
  mcp.json              ← personal MCP servers (same format as plugins/gal-core/mcp.json; gal read-only)
  conventions/
    <lang>.md           ← personal coding-style convention file (gal read-only; see below)
```

**Enable:** presence-based — there is no config flag. Placing a file at the personal root is itself the opt-in; an absent directory or file keeps every render byte-identical to a Core-only render.

**Projection rules:**

- Personal skills are merged after core content; any personal skill whose name collides with a core skill is silently skipped (core-wins).
- Personal MCP servers are merged into the canonical `.mcp.json`; a personal server whose name collides with a core server is skipped (core-wins).
- After placing files: run `gal refresh` to re-project the personal layer (it merges during the canonical-root render). `gal doctor` reports the count of personal skills, personal convention files, and any core-collision skips.

### Personal Coding-Style Conventions

GAL Core ships no owner-personal coding-style convention — only `rust.md`, GAL's own dev standard for its own `crates/` workspace. Your house style (C#, Go, TypeScript, or anything else) reaches downstream repos through one of three sources, and you pick whichever fits what's already on your machine:

**Source 1 — personal convention file (always-on, per-repo).** Copy the shipped example at `plugins/gal-core/templates/csharp-convention.example.md` to `~/.gal/local/conventions/csharp.md` — this exact destination path — and adapt its content to your own style. Then run `gal init` in any target repo whose `Language` row (in `.dev/project.md`) matches. The file is injected into that repo's adapters the same way a gal-core convention would be, carrying a distinct `personal-conventions/` source label.

**Source 2 — installed agent-plugin detection (read-only, zero setup).** If you already have an official language plugin installed in your coding agent (for example a Flutter/Dart plugin in Claude Code), just run `gal init` in the target repo. GAL scans the installed skill roots (Claude plugin cache, shared agents skills, Copilot skills, agy skills), matches the plugin's skill name/origin against the repo's `Language` row, and renders a **Detected Language Skills** reference block — the skill's name, its origin path, and a standing "load this first" instruction. GAL never copies the skill's content. A newly installed plugin is only picked up on the *next* `gal init` — and the block is an always-on standing instruction, but actual loading is the agent's job, not GAL's (an explicit honesty boundary, not a guarantee).

**Source 3 — personal skill (on-demand).** Write a `SKILL.md` with a clear `description` under `~/.gal/local/skills/<name>/`, run `gal refresh`, then restart Claude Code (other agents auto-pick-up on their next read). This is loaded by the agent when named, not injected always-on like sources 1 and 2.

**Which source to use:** prefer source 2 when the official plugin is already installed — it needs zero setup. Otherwise, source 1 gives you an always-on personal style with full control over content. Source 3 is for anything you want the agent to consult on demand rather than always.

**Miss contract:** if a repo's `Language` row matches nothing in any of the three sources, the adapters simply carry no language-specific convention or reference block. This is a silent, by-design skip — not an error.

**Plugin lifecycle stance:** GAL only detects and references installed plugins (source 2) — it never installs, updates, or removes them. That lifecycle belongs to the hosting agent today (Claude Code, Codex, etc.), and to the future **ccync** product for cross-agent plugin management.

Add a `| Personal Conventions | off |` row to a repo's `.dev/project.md` Tech Stack table to disable sources 1 and 2 for that repo (recommended for public repos, so tracked adapters never embed owner-machine content); source 3 is unaffected since it is never injected always-on.

### Model Routing

- Paste the shape shown in [Headless Executor Routing](#headless-executor-routing) into `~/.gal/config/config.json` under the `executorRouting` key.
- Change role-to-executor mappings in `config.json#executorRouting`. Role definitions and cross-model policy are in `plugins/gal-core/workflows/coding.md`.

### Local Secrets, Paths, and Doc Language

Put secrets, absolute paths, and machine-specific values in `~/.gal/config/config.json`. Documentation language follows the same ownership split:

- `planLanguage` (in `config.json`) is machine-local and optional; it sets the default output language for `.dev/plans/*.md` and `.dev/research/*.md` when no explicit directive is given. Resolution: explicit directive → `planLanguage` → prompt-language auto-detect → fallback `en`.
- `PROJECT_LANGUAGE` (tracked project metadata) controls the canonical language for main docs.
- Translations live under `docs/i18n/<lang>/` as `<name>.<lang>.md` (see [developer guide → Documentation Conventions](devguide.md#documentation-conventions)).
- `.dev/plans/*.prompt.md` stays English-only for cross-model stability.

#### Non-English planLanguage: Draft Lifecycle

When `planLanguage` is **not** English, GAL keeps the plan's technical meaning stable across sessions and cross-model edits by splitting planning into a **three-layer authority** (full rationale: [architecture D20](architecture.md); contract: `workflows/coding.md` → Planning-Language Authority):

- **EN semantic draft** — `.dev/plans/<slug>.en.md`, the single authority for planning-stage technical meaning, in English. It is a tracked file (repo-owned, recoverable across sessions/machines), **not** a source plan — it never appears in `/gal status` active plans and is never a `/gal pipeline` or `/gal finalize` target.
- **Localized source plan** — `.dev/plans/<slug>.md`, what you read and hand-edit, rendered from the draft in your `planLanguage`. Machine anchors (headings, file paths, task/test IDs, review verdicts, metadata keys) stay English by design; only narrative prose is in your language.
- **English execution prompt** — `.dev/plans/<slug>.prompt.md`, generated at `/plan-to-prompt`.

**Lifecycle:** you may hand-edit the localized source freely. On the next planning-stage command (`/deep-planning` / `/refining-plan` / `/plan-to-prompt`) GAL detects the edit (a rendered-source-hash mismatch) and **stops at a read-only reconcile preflight** — it merges your intent back into the EN draft before continuing, never silently overwriting your edits. After `/plan-to-prompt` the EN draft is deleted; the equivalence proof lives on as the source plan's own inline `prompt-hash` / `equivalence-verdict` metadata fields — there is no separate `.equiv.md` receipt file, so a non-English plan has exactly two tracked files (`.md` + `.prompt.md`). English (`en`-prefix) `planLanguage` pays none of this cost: the source plan is its own authority, no draft, no reconcile.

### Command Skill Local Overlays

For a machine-local customization of a command skill, create `plugins/gal-core/commands/<command>/SKILL.local.md`:

- It is gitignored and treated as user-owned machine-local input.
- The bake step appends `SKILL.local.md` into the generated `SKILL.md`.
- Do not edit `plugins/gal-core/commands/<command>/SKILL.md` directly — it is generated and will be replaced.
- Keep `SKILL.local.md` to additional instruction content only; no second frontmatter block.

### local-notes Routing

External notes are machine-local and optional. GAL separates repo-owned state from user-owned notes; Core behavior never depends on a private note store. The portable contract is [local-notes](collaborative-tools/local-notes.md): app-agnostic, default-off, and only used when a machine-local backend is ready. Obsidian remains one possible backend via the built-in `obsidian` CLI and related local wiring, but it is no longer the Core default or the only documented route.

Repo-owned research stays in `.dev/research/` by default. GAL scratch output uses the repo-relative `.dev/tmp` convention; `.gitignore` already ignores `.dev/*`, so temporary artifacts stay machine-local without adding another config key.

See [local-notes](collaborative-tools/local-notes.md) for the portable contract and backend examples.

### Working Hours

Working-hours enforcement is disabled by default. To respect your own workday boundary, configure `~/.gal/config/config.json` under `workingHours`: `enabled: false` keeps it off; `workdayStart`/`workdayEnd` describe your window; `wrapUpTime` starts reminders and the shutdown window; `hardStopTime` is where agents refuse further work. These are machine-local preferences, not tracked repo policy.

### Remote Execution (SSH Dispatch Lane)

Cross-machine execution is configured entirely in `~/.gal/config/config.json#executorRouting` — add `sshTarget` + `remoteWorkdir` to a role entry (see [Headless Executor Routing](#headless-executor-routing) above). There is no separate node-alias config file; see `docs/remote-execution.md` for the full contract, prerequisites, and safety boundaries.

### MCP Overrides

The tracked GAL source in `plugins/gal-core/mcp.json` (plus bundle manifests) is the single source of MCP servers. Machine-specific differences are supplied as `${ENV_VAR}` placeholders resolved at projection time from `~/.gal/config/config.json` (its `secrets` map and machine paths) — there is no separate override file.

For Playwright MCP, keep the tracked entry conservative and machine-agnostic; express local-only browser behavior (headed mode, viewport/device emulation, storage-state paths, output dirs, persistent profiles, extension/CDP wiring) through `${ENV_VAR}` placeholders in the tracked manifest, resolved from `config.json`:

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

Example for two Postgres databases on one machine:

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

Add matching values to the `secrets` map in `~/.gal/config/config.json`; `${ENV_VAR}` placeholders in MCP configs are resolved at runtime. Installed runtime MCP configs stay user-owned: VS Code user `mcp.json`, AGY plugin-root `~/.gemini/antigravity-cli/plugins/gal/mcp_config.json`, Codex `config.toml` `[mcp_servers.*]`, Claude user-scope via `claude mcp`. Do not commit storage-state files, persistent profiles, browser artifacts, or secret-like local files.

### Headless Executor Routing

`config.json#executorRouting` (read by the `gal` binary) has **two distinct consumer classes**, keyed off the same role names but reached by different code paths:

- **(a) Dispatch offload** — pipeline roles `CODER` / `TESTER` / `AUDITOR`. GAL can offload pipeline phases (implement / test / audit) to a secondary headless CLI instead of the conversation loop, via the dispatch path described below.
- **(b) Projection** — consult roles `ARCHITECT` / `ANALYST` / `DESIGNER` / `RELEASER`, resolved by golem-name uppercasing in `crates/projection` (`resolve_codex_routing`, keyed off `CONSULT_GOLEM_NAMES` in `crates/projection/src/codex_agent.rs`). This mapping is used **only** to resolve the Codex native-subagent model/`effort` for that consult role — it is never headless-dispatched.

Configure the `executorRouting` subtree in `~/.gal/config/config.json`. Roles are grouped **by consumer** into two objects — `pipeline` (dispatch offload) and `planning` (projection consult) — plus the shared `executors` default-model block:

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

The two groups have closed role allowlists: `pipeline` ← `{CODER, TESTER, AUDITOR}`, `planning` ← `{ARCHITECT, ANALYST, DESIGNER, RELEASER}`. The flat shape (a role key directly under `executorRouting`) is **retired** — a leftover flat entry is not parsed and produces a visible retirement warning naming the key and its target group (no alias, no fallback); a role in the wrong group, or an unknown role, is skipped with a warning naming the correct group.

**`executors` block** — defines the default model for each tool. A role entry that omits `model` inherits the default from `executors[executor]`. An explicit `model` on a role always takes precedence. The bin warns in the executor log if a role has neither.

**Remote dispatch (`sshTarget` + `remoteWorkdir`)** — dispatch-class (a) roles only (`CODER`/`TESTER`/`AUDITOR`, under the `pipeline` group). Add both fields to a role entry to run that phase over SSH instead of locally:

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

Both fields are required together — a half-configured entry (only one of the two) loads with a warning and fails loud at dispatch time (`remote-missing-workdir`). Remote runs the raw agent CLI only — no `gal` needed on the remote host. `remoteWorkdir` must be a **GAL-dedicated checkout**: after a successful remote run the file-return flow fetches the diff, applies it locally, then destructively resets the remote checkout (`git reset --hard && git clean -fd`) back to the control node's HEAD. See `docs/remote-execution.md` for the full contract, prerequisites, and safety boundaries.

**Per-role `effort` key** — optional reasoning-intensity hint (e.g. `"effort": "high"`), honored by **both** consumer classes:

- **Dispatch-class (a)** roles (`CODER`/`TESTER`/`AUDITOR`): the dispatch layer maps `effort` to the resolved executor's native reasoning flag before spawn:

  | Executor | `effort` → native flag |
  | --- | --- |
  | claude | `--effort <value>` |
  | codex | `-c model_reasoning_effort="<value>"` |
  | copilot | `--reasoning-effort <value>` |
  | opencode | `--variant <value>` |
  | agy | unsupported |

  It is **fail-closed**: an executor that cannot honor `effort` (agy, or any future adapter that has not opted in) degrades the dispatch with `unsupported-effort` before spawn; a value outside `[A-Za-z0-9._-]+` degrades with `invalid-effort`. Neither dispatches.

- **Projection-class (b)** roles: read by `resolve_codex_routing` (`crates/projection/src/lib.rs`) to set the Codex native-subagent's `model_reasoning_effort` (unchanged).

`effort` is a reasoning-intensity hint, **not** a permission control and **not** a model selector: it never implies a paid or named model, is set as `"effort": "high"` on the role (never as a raw `extraFlags` bag), and its semantics are provider-specific (GAL passes the value through as a typed hint).

**Adapter behavior notes** — the built-in headless flags are adapter-owned in Rust (no raw flags in the routing schema):

- **OpenCode** dispatches `run --format json --auto --agent build -m <model>`. `--auto` is the current permission-bypass flag (the older `--dangerously-skip-permissions` was removed from the CLI); `--agent build` selects the write-capable agent (the default/`plan` agent is read-only and silently writes nothing).
- **Copilot** always adds `--no-custom-instructions` + `--disable-builtin-mcps` so its headless prompt-mode does not overflow the static context before writing the receipt; the disabled MCP-server names come from Copilot's own `~/.copilot/mcp-config.json` (missing → none; unreadable/malformed → none + a warning). Default tools are kept (no `--available-tools write`). Copilot Free is **auto-only** — it stays `model: auto` and remains local-only. This is what lets `gal doctor --executor-smoke --executor copilot` write a receipt (reach `completed`) on a Copilot Free account instead of degrading to `NO_RECEIPT` from context overflow.

**Role table** (consumer + purpose):

| Role | Class | Purpose |
| --- | --- | --- |
| `CODER` | (a) dispatch | Writes implementation code following a plan — must differ from `TESTER` |
| `TESTER` | (a) dispatch | Writes tests from plan spec + public API only — must differ from `CODER` |
| `AUDITOR` | (a) dispatch | Audits deep performance and security — must differ from `CODER`; tier ≥ `CODER` |
| `ARCHITECT` | (b) projection | Adversarial plan review — trade-offs, over-engineering, bugs |
| `ANALYST` | (b) projection | Business logic review — ROI, domain correctness, user impact |
| `DESIGNER` | (b) projection | UX/UI + DevEx review |
| `RELEASER` | (b) projection | Release-flow design (consult, read-only) |
| `RESEARCHER` | — | Investigate unknowns, synthesize findings, cross-review sources |

Research is **in-process, never headless-routed** — `RESEARCHER` has no `executorRouting` entry; `/gal research` and `/gal deep-research` run in the conversation loop with their own cross-model verification (the golem is unchanged). Valid executors: `claude`, `codex`, `opencode`, `copilot`, `agy`. Omit a role to keep it in the conversation loop; delete the file to disable routing (the bin then emits a `--- GAL DISPATCH ---` text fallback). Role definitions and cross-model policy (CODER≠TESTER, etc.) live in `plugins/gal-core/workflows/coding.md`.

> ⚠️ **SECURITY WARNING — bypass-permission.** Headless executor adapters invoke secondary CLIs with `--dangerously-skip-permissions` (Claude Code, Antigravity/agy), `--auto` (OpenCode), `--allow-all` (Copilot), or `-s workspace-write` (Codex), granting **full trust** over the local filesystem and terminal — equivalent to no sandbox. Enable executor routing only on machines and in environments you fully trust, and never when the repo or agent contracts come from untrusted sources. The spec forbids the secondary CLI from running `git commit`/`git push`, but that is an instruction, not a technical enforcement.

#### Pipeline Input Resolution

`gal pipeline <path>` accepts three input kinds and resolves each deterministically by the path's conventional location/suffix:

| Input | Recognized by | Behavior |
| --- | --- | --- |
| Execution prompt | `*.prompt.md` | **Preferred.** Reads the prompt body and materializes the per-(task,phase) task spec internally (scoped to one task — `--task <T-NN>`, else the first task bullet), then dispatches. |
| Source plan | a `.dev/plans/<slug>.md` path | Switches to the paired `.dev/plans/<slug>.prompt.md`. If that prompt does not exist, it errors and points at `/refining-plan`, then human approval in `## Approval`, then `/plan-to-prompt` — it never dispatches against a source plan. |
| Raw task spec | anything else (e.g. a `.dev/generated/task-specs/…` file or any other `.md`) | Fed to the dispatcher as-is (back-compatible default). |

The **offload decision is keyed on `(task, phase, routing)` only — never on `.dev/state.md`.** A routed executor for the phase role is sufficient to offload, independent of how `state.md` lists active plans. `state.md` stays a human record: the AI orchestrator still reads it for three-surface convergence and resume, but the Rust dispatch gate does not parse it.

#### Inspecting A Dispatch — Receiving And Reviewing Results

Every dispatch leaves **two** durable traces. Use the right one for the job:

**Tier 1 — GAL executor log (primary; uniform across all tools).** Each dispatch writes `<repo>/.dev/executor-logs/<epoch>-<task>-<phase>-<executor>.log`. The header records terminal state, exit code, actual model, git branch/HEAD, and the captured provider `session_id`; the `---STDOUT---` section captures the **full** provider event stream (every agent message, command execution, file change, and token usage). This is the canonical audit trail — to review *what the executor did*, read this log. It is identical in shape for all five tools, so one location serves every provider.

This aligns with the GAL memory contract: **provider-local chat history is advisory, not authoritative.** The executor log is the repo-owned, reviewable record.

**Tier 2 — provider-native session resume (secondary; for continuing/branching).** The `Dispatch:` marker and the log header record a resumable `session_id`. Use it when you want to **continue or branch** the conversation inside the provider's own UI. Each tool stores and surfaces headless sessions differently — the commands are **not** uniform:

| Executor | Native-view / resume command | Default visibility of headless session |
| --- | --- | --- |
| **claude** | `claude --resume <session_id>` | listed |
| **codex** | `codex resume <uuid>` (UUID bypasses the filter) | **hidden** — `codex exec` creates a *non-interactive* session that the `codex resume` picker hides by default; use `codex resume --include-non-interactive` (add `--all` to disable cwd filtering) to see it in the picker. Rollout file: `~/.codex/sessions/YYYY/MM/DD/rollout-…-<uuid>.jsonl` |
| **opencode** | `opencode run -s <session_id>` (continue) · `opencode export <session_id>` (dump JSON) · `opencode session list` (browse) | listed — `run` sessions appear in `opencode session list` and the TUI; cannot be hidden |
| **copilot** | `copilot --resume=<session_id>` | stored in `~/.copilot/session-store.db`; resume by id (no public list command) |
| **agy** | `agy --conversation <uuid>` | stored under `~/.gemini/antigravity-cli/brain/<uuid>/` (Windows); no list subcommand — browse by id |

**Rule of thumb:** to *audit* a dispatch, read the Tier-1 executor log (uniform, always present). To *resume* a dispatch in its native tool, use the Tier-2 command for that executor. The "hidden but resumable" behaviour you may notice with `codex` is that tool's own default, not a GAL setting; only `codex` hides headless sessions by default, and all five remain resumable by id regardless.

## Machine Operations

### When To Rerun Setup

Repo-local adapters (CLAUDE.md, AGENTS.md, GEMINI.md, .github/copilot-instructions.md) are regenerated by `gal init`. The binary on your machine is managed by your package manager.

```bash
# Upgrade the binary
cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli  # cargo
winget upgrade Monkey1Wizard.GAL                                                   # Windows
brew upgrade gal                                                                   # macOS

# Rebuild the canonical root + runtime projections (after a binary reinstall or broken derived content)
gal refresh

# Check setup health
gal doctor
```

### Internal Binary Subcommands

A few `gal` subcommands are **internal tooling invoked by control-plane procedures**, not part of the public `/gal` command surface. They do **not** change the public-command count — the control plane stays **11 commands**; these are binary helpers in the same class as `gal naming-gate`:

- **`gal finalize-check <plan-or-prompt> --receipt <path>`** — the deterministic precondition engine `/gal finalize` runs before landing a plan. It runs 8 checks and emits a machine receipt that is the sole pass-basis. The first 5 checks are: authoritative-command + naming-gate, commit-existence, three-surface checkbox agreement, cited-test-name existence, adapter-render idempotency ×2. The cited-test-name existence check is **plan-type-aware**: when the target plan's `## Test Plan` table declares only `manual` probes (every `Type` cell exactly `manual`), there is no fn-citation contract, so it vacuously passes with a `skipped:` summary rather than harvesting `## Test Results` for fn names; a missing / mixed / unknown-`Type` table stays fail-closed. The 3 additional read-only checks are:
  - **`finalize-mode` (check(f))** — records `worktree` or `already-on-main` based on `git branch --show-current`; state is always `Pass` (informational, never a gate). `/gal finalize` reads the `summary` field to decide whether merge + teardown apply.
  - **`executor-log-scan` (check(g))** — walks `.dev/executor-logs/*.log`, reads each file's `terminal_state` header; any state other than `completed` → `Fail` (covers `no-receipt` / `timeout` family / `disconnected-partial` / `unavailable`); no log files → `NotRun` (non-zero exit). This is the mechanized evidence for the pre-review pipeline-integrity assertion.
  - **`durable-layer-commit` (check(h))** — reads the last `*(hex)*` hash from any `[x]` task line and asserts its git existence via `git cat-file -e`. Summary: `exists: <hash>` / `missing: <hash>` / `not yet recorded`. **The binary only asserts existence — it does NOT authorize deletion.** Deletion of plan files stays orchestrator-executed after ORCHESTRATOR reads the receipt and confirms the hash independently.

  It is not something you normally run by hand; `/gal finalize` invokes it.

- **`gal refresh`** — idempotent machine-level rebuild of the canonical plugin root (`~/.gal/plugins/gal`), the parent Claude marketplace manifest (`~/.gal/plugins/.claude-plugin/marketplace.json`), and per-runtime projections (copilot/codex/agy/gemini/opencode). Run this after installing a new `gal` binary, after breaking or deleting derived content, or whenever a runtime is missing GAL commands. All output is rebuildable-derived from source; no backup is needed.

  **Boundary:** `gal refresh` writes only GAL's own `~/.gal` tree. It does **not** write `~/.claude/plugins/cache` or `installed_plugins.json` — those are Claude-managed. Claude re-picks up the updated plugin after a **restart** or **re-add** (`claude plugin add ~/.gal/plugins`). This is the documented Claude-side step for picking up changes after a refresh.

  **All-5-runtime projection:** `gal refresh` projects to **all 5 runtimes** by default (copilot/codex/agy/gemini/opencode). Selective per-runtime projection is a future enhancement.

  ```sh
  gal refresh
  ```

- **`gal pipeline-preflight <prompt> [--receipt <p>]`** — Step-1 gate run by `/gal pipeline` before the task loop. Checks: `## Tasks` has ≥1 well-formed T-NN entry; `## Test Plan` is present; no root-level `BLOCKING` marker; `Workflow` ≠ `DONE`; prompt is the active plan in `.dev/state.md`. Also reads and reports the resume cursor. Receipt is the sole pass-basis; `fail` or `not-run` → STOP + Interrupted Phase block.

- **`gal boundary-check <prompt> --task T-NN [--receipt <p>]`** — 2c gate run before every implementation commit. Resolves the task's affected-files allowlist with the shared `task_spec` parser — the same set the dispatch spec shows the executor: backtick paths named in the task block, falling back to the `## Files to Create or Modify` section — and compares it against `git diff --name-only HEAD` + `git status --porcelain`. A standard-template prompt already carries this; **no manual `Affected:` clause is required.** **Critical contract:** when the task names no affected files at all (no task-block paths and no `## Files to Create or Modify` entries) → `not-run` (non-zero, never allow-all). Out-of-allowlist file → `fail`. Receipt is the sole pass-basis; `fail` or `not-run` → STOP before the commit.

- **`gal pipeline-converge-check <prompt> --task T-NN [--receipt <p>]`** — 2g closeout gate run after the three-surface write-backs. Three checks: source plan and prompt checkbox agreement + T-NN `[x]` on both surfaces; task `*(hash)*` commit note resolves to a real commit; `## Status` `Current Task:` is cleared. Receipt is the sole pass-basis; `fail` or `not-run` → STOP + Interrupted Phase block.

- **`gal planning-check <source-plan> [--receipt <p>]`** — planning-stage handoff gate for source plans. Verifies `## Open Questions` is `None`, required sections exist, `<!-- ARCH_REVIEW: CLEAR -->` is present, naming-gate is clean, and `planLanguage=zh-TW` plans actually contain CJK text.
- **`gal prompt-check <prompt> [--receipt <p>]`** — execution-prompt anchor gate. Verifies compressed prompts still carry the parser anchors consumed by pipeline/finalize (`## Status`, `Current Task:`, `## Tasks`, unchecked `T-NN`, `## Test Results`, `## Review Results`, and all four review subsections).
- **`gal refining-check <source-plan> [--receipt <p>]`** — refined-plan structure gate. Verifies task count is 1–99 (**zero tasks → fail**; a plan with no `T-NN` bullets is a scope statement, not an executable plan), T/TP IDs are well-formed + unique + paired, task-referenced file paths exist, and `<!-- ENG_REVIEW: CLEAR -->` appears only on a structurally valid plan.

- **`gal restore [--yes]`** — revert the GAL source repo to the last `/gal finalize`-verified known-good state. Use when you want to discard local changes (committed or uncommitted) and return to a clean verified baseline.

  **Baseline = `gal-last-good` tag.** `/gal finalize` writes a lightweight git tag `gal-last-good` to the landing commit after every successful finalize (Sequence 6). `gal restore` reads that tag as the sole baseline; if the tag is absent (bootstrap: no finalize has run yet), the command refuses with an actionable message — it never falls back to HEAD.

  **Two-layer restore.** `gal restore` converges both layers:
  - **Source layer** — `git reset --hard gal-last-good` + `git clean -fd` (worktree **exactly** equals the marker commit; tracked files added after the marker and all untracked files are removed).
  - **Derived layer** — delegates `gal refresh` to rebuild the canonical root + runtime projections.

  **Zero-loss backup net.** Before any mutation, `gal restore` creates a `gal-restore-backup-<ts>` branch at the current HEAD. This backup is a **hard precondition** — if branch creation fails, restore aborts without touching the worktree.

  **Explicit confirmation required.** Without `--yes`, the command prints the discard scope (uncommitted count + commits ahead of the marker) and exits with a usage error. Provide `--yes` to proceed.

  **Bootstrap note.** The `gal-last-good` tag does not exist until the first `/gal finalize` after this feature lands. Until then, `gal restore` will refuse with a clear message. Run any plan through the full pipeline + finalize to establish the baseline.

  **Scope.** `gal restore` reverts the GAL *source* repo only (source + derived layers). It does not copy machine-local config across machines.

  ```sh
  # Dry-run: see what would be discarded
  gal restore

  # Proceed: restore to last verified baseline
  gal restore --yes
  ```

### Commit Message Assembly (`gal commit-msg`)

`gal commit-msg` is the deterministic, no-hijack commit helper the `git-commits` skill and `git-commit-msg` command call. Its `type`/`scope` classification derives **only** from changed file paths + git status — never from diff/body keywords — so a body that happens to contain words like `fix` or `git-commit-msg` cannot hijack the classification. It has three modes:

- **`gal commit-msg --context`** — the single front-end call a message-drafting consumer makes. It does all the pre-work and prints up to five blocks, omitting any that are empty: `FILES` (name-status), `BASELINE` (the deterministic `type(scope)` header), `PLANS` (per staged source plan: slug, verb, title, and truncated `## Goal`), `PROMPTS` (staged execution-prompt slugs + count), and `CHANGES` (`@@` hunk headers for non-plan code files). This replaces the old "let the model read the raw staged diff itself" flow — higher signal (the model sees each plan's real intent) at lower token cost (a converged summary, not the whole diff). Empty staging prints `No staged changes.`
- **`gal commit-msg --print`** — the baseline-only lane: prints just the deterministic `type(scope): subject` header. Retained for the commit-msg hook, the `gal`-not-on-PATH fallback, and the OpenCode legacy path. `--print` wins if both `--print` and `--context` are passed.
- **`gal commit-msg <file>`** — the git `commit-msg` hook: non-destructive, fills a blank message from staged changes and never clobbers an authored one.

**Consumer split (documentation contract, not binary-enforced — the binary does not know its caller):** the `git-commit-msg` command is **message-only** (produces wording, never runs `git commit`); the `git-commits` skill produces the message **and** executes the commit when the user's intent is to commit. All five runtimes call `--context`; the OpenCode path is rendered from a hardcoded string in `crates/projection/src/support.rs::render_opencode_command` that is kept in lockstep with the command template.

### Executor Self-Test (`gal doctor --executor-smoke`)

Plain `gal doctor` never calls a headless coding-agent CLI — it is a cheap, non-calling health check. `gal doctor --executor-smoke` is the opt-in, durable, repeatable self-test that actually exercises the same headless dispatch path a real pipeline task uses (`dispatch::run::run_dispatch`), for all five supported CLIs: `codex`, `claude`, `copilot`, `agy`, `opencode`.

```sh
# Self-test all five agents (default --transport local)
gal doctor --executor-smoke

# Filter to specific agents (repeatable)
gal doctor --executor-smoke --executor codex --executor claude

# CI/scheduler use: machine-readable JSON, non-zero exit on any non-pass row
gal doctor --executor-smoke --json --strict

# Bound the per-agent timeout (seconds; default 300)
gal doctor --executor-smoke --timeout 60

# Write evidence somewhere other than the default gitignored root
gal doctor --executor-smoke --report-dir /path/to/evidence
```

**Real-path fidelity.** The self-test drives the exact same code path a real `implement`/`test`/`audit` dispatch uses — routing entry, readiness gate, per-executor adapter invocation, spawn, terminal-state classification, receipt verification, executor-log write, session-id extraction — via a synthesized single-executor routing file (never your real `config.json#executorRouting`) and a tiny, harmless, pipeline-shaped task spec whose only job is to write a one-line receipt. There is no separate spawn-only probe to drift out of sync with real dispatch behavior.

**Report locations.** Each run persists under a gitignored, timestamped run directory — by default `.dev/executor-smoke/runs/<utc-run-id>/local/` — containing:

| File | Purpose |
| --- | --- |
| `summary.json` | Machine-readable report (the `--json` output). |
| `summary.md` | Human-readable table (the default table output). |
| `logs/` | Per-agent executor logs (same header format as `.dev/executor-logs/`, but run-scoped — **never** written to the root `.dev/executor-logs/` directory, so a stale self-test failure can never break `gal finalize-check`'s `executor-log-scan`). |
| `receipts/` | Per-agent smoke receipts proving the real call actually wrote back. |
| `.dev/executor-smoke/latest.json` | Always overwritten with the most recent run's `summary.json` — the fast "what happened last time" pointer. |

**Status meanings:**

| Status | Meaning |
| --- | --- |
| `PASS` | Real dispatch completed and the receipt was verified. |
| `NOT_INSTALLED` | The CLI is not on PATH. Install it. |
| `NOT_AUTHENTICATED` | Confirmed unauthenticated — the self-test never launches an unauthenticated tool into an interactive login flow. Run the tool's own login command. |
| `AUTH_UNKNOWN` | Readiness could not be confirmed either way; the self-test still attempts a bounded call and reports that call's real outcome as the final status. |
| `UNSUPPORTED` | An `--executor` name outside the five supported agents — a configuration mistake, never dispatched. |
| `CONFIG_ERROR` | A pre-dispatch problem, most commonly an unsafe `--report-dir` (see below) — always caught **before any write**. |
| `CALL_FAILED` | The executor exited non-zero. Inspect the run's executor log. |
| `NO_RECEIPT` | The executor exited 0 but never wrote the expected receipt — exit 0 alone is never treated as success. |
| `TIMEOUT` | The bounded timeout elapsed before the call finished. Widen `--timeout` if the tool is just slow, or investigate if it never produced output (a likely hung interactive prompt). |

**`--report-dir` safety.** A target resolving to the repo root itself, or under a durable/tracked surface (`docs/`, `plugins/`, `crates/`, `.github/`, `.claude/`, `.agents/`, `src/`, or a root file like `README.md`/`CLAUDE.md`/`AGENTS.md`/`GEMINI.md`), is rejected as `CONFIG_ERROR` **before any log or receipt is written** — the self-test can never contaminate a durable or generated surface.

**Exit behavior.** `--strict` makes the exit code non-zero whenever any requested row is not `PASS` — the shape a CI job or OS scheduler needs. Without `--strict`, `gal doctor --executor-smoke` exits 0 as long as the run itself completed (even if individual rows report install/auth/call problems) — the report is the source of truth, not the exit code.

**Not a daemon.** This is a repeatable CLI command, not a background service — run it by hand, wire it into CI, or point an OS scheduler (cron / Task Scheduler) at it on whatever cadence you want.

#### Remote (SSH) Self-Test (`--transport ssh`)

The same self-test can validate coding-agent readiness on a remote machine over SSH, reusing the identical smoke core, report schema, renderer, run store, and status classification — only the transport differs. It drives the **same** real `run_dispatch` remote branch a user-issued pipeline SSH dispatch uses (HEAD/clean guard, unsupported-CliFlag rejection, receipt fetch, terminal-state sync); the SSH-specific additions are a non-interactive reachability precheck and remote install/auth probes evaluated **on the remote machine**, never inferred from the control node.

```sh
# Self-test all five agents on a remote host over SSH
gal doctor --executor-smoke --transport ssh --ssh-target <ssh-target> --remote-workdir <dedicated-checkout>

# CI/scheduler use: machine-readable JSON, non-zero exit on any non-pass row
gal doctor --executor-smoke --transport ssh --ssh-target <ssh-target> --remote-workdir <dedicated-checkout> --strict --json

# Filter to one agent, keep run evidence under the default gitignored root
gal doctor --executor-smoke --transport ssh --ssh-target <ssh-target> --remote-workdir <dedicated-checkout> --executor claude
```

- `--ssh-target <ssh-target>` is an SSH host (a `user@host` or a configured `~/.ssh/config` alias) reachable **non-interactively** (key-based, `BatchMode=yes`). Use a placeholder like `<ssh-target>` in any shared example — never a literal machine hostname.
- `--remote-workdir <dedicated-checkout>` is **required** and must be a **dedicated** GAL checkout on the remote machine. Both flags are mandatory for `--transport ssh`; omitting either fails fast with `CONFIG_ERROR` before any run. The shared `--strict`/`--json`/`--timeout`/`--executor`/`--report-dir` behavior is unchanged.

**Remote prerequisites.** The remote checkout must be at the **same** git HEAD as the control node and clean (the real remote guard fails loud otherwise — GAL never self-syncs it). The remote machine runs the raw agent CLIs only; a remote `gal` is neither required nor part of the contract. Remote availability and authentication are evaluated on the remote machine's own non-interactive `PATH`/state, not the control node's.

**Report location.** Remote runs persist under a gitignored, timestamped `.dev/executor-smoke/runs/<utc-run-id>/ssh/` (the `transport=ssh` subdirectory) with the same `summary.json` / `summary.md` / `logs/` / `receipts/` shape as local, plus the shared `latest.json` pointer. Remote smoke evidence never lands in the root `.dev/executor-logs/`.

**Remote-only status extensions** (they never occur on the local transport and do not change the local report shape):

| Status | Meaning | Remediation |
| --- | --- | --- |
| `SSH_UNREACHABLE` | The control node could not open a non-interactive SSH session to `<ssh-target>` (checked before any executor probe). | Check the target, network, and key-based auth (`BatchMode`). |
| `REMOTE_GUARD_FAILED` | The remote workdir is missing, unsafe, dirty, or not at the control node's HEAD. | Re-sync the dedicated remote checkout to the control node's HEAD and ensure it is clean. |
| `REMOTE_FETCH_FAILED` | The remote process may have run but fetching its receipt back over SSH failed (distinct from a genuinely missing/empty remote receipt). | Inspect the run's executor log and the remote receipt path. |

Remote `copilot` reports `UNSUPPORTED` while its CLI-flag spec delivery cannot be safely forwarded over SSH — it is never counted as a pass and never omitted. All shared statuses (`PASS`, `NOT_INSTALLED`, `NOT_AUTHENTICATED`, `AUTH_UNKNOWN`, `CONFIG_ERROR`, `CALL_FAILED`, `NO_RECEIPT`, `TIMEOUT`) keep their local meaning; `PASS` still requires terminal `completed` **plus** a fetched, non-empty receipt.

**Recurring remote validation.** GAL does not run a remote daemon. For continuous remote validation, point CI, cron, or Task Scheduler at the same strict-JSON command on whatever cadence you want; each run appends a timestamped report and refreshes `latest.json`. Because the guard fails loud on HEAD drift, re-sync the dedicated remote checkout before a scheduled run.

## Install Status & Channels

Machine-local settings are preserved in `~/.gal/config/config.json`. Use `gal doctor` to check setup health. After reinstalling the binary, run `gal refresh` to rebuild the canonical root + runtime projections.

## Golem Agents

GAL's specialist agents ("golems"). This is the **user-facing** view — what each does, how it differs from its neighbours, and when to reach for it. The authoritative roster + classifications live in [plugins/gal-core/agents/agents.md](../plugins/gal-core/agents/agents.md); the workflow semantics live in [plugins/gal-core/workflows/coding.md](../plugins/gal-core/workflows/coding.md).

| Golem | What it does | When to use it |
| --- | --- | --- |
| `golem-architect` | Adversarial plan review — trade-offs, over-engineering, bug surface, dependency/API risk | `/deep-planning`, or `/gal architect` to pressure-test a design before building |
| `golem-analyst` | Business-logic review — ROI, domain correctness, user impact | When a change touches pricing, permissions, eligibility, or other customer-visible rules |
| `golem-designer` | UI & UX / experience design (incl. developer-facing DevEx), design system, accessibility, live UI audit | Customer-facing layout/state/component work, or developer-facing DevEx (CLI DX / API / workflow friction) |
| `golem-researcher` | Local-first investigation, cross-source synthesis, reference-ready findings | `/gal research` / `/gal deep-research` for an evidence-backed answer |
| `golem-implementer` | Writes implementation code for one approved task, atomic commits | The CODER phase inside `/gal pipeline` |
| `golem-tester` | Spec-driven tests + real-browser QA, written from the plan spec (not the implementation) | The TESTER phase — independent verification, a different model from the implementer |
| `golem-auditor` | Deep performance + security audit of a single task, and standalone branch-audit | The AUDITOR phase, or `/gal auditor` to audit a branch |
| `golem-debugger` | Scientific-method bug investigation with freeze discipline and root-cause confirmation | Any time a bug needs disciplined investigation before a fix |
| `golem-steward` | Documentation structure — structure map, code→doc drift, knowledge extraction → durable layer (`README.md` + `docs/` excl. `plans/`+`research/`), figure sync | `/gal steward`, or at planning-open / refining-end / pipeline-closeout |
| `golem-releaser` | Planning-stage release-flow designer (consult): researches APIs/CICD tools, designs a release/devops flow, emits design advice | `/gal releaser` (isolated) or `/gal discuss releaser` (in-context); call before `/planning release-<slug>` |

**Checking-role triangle.** Checking responsibilities form a triangle: the **ORCHESTRATOR** (the pipeline itself) owns the per-task correctness gate, the end-of-run goal-backward verification, and plan lifecycle close; the **AUDITOR** owns single-task deep performance + security; the **STEWARD** owns documentation structure.

**Steward lifecycle split (both mandatory, different jobs by phase).** The steward runs at planning and at finalize, but does a *different* job at each — the phases are not interchangeable:

- **At planning (`/deep-planning` Step 3e) — plan-document structure only.** The steward reviews the plan file itself (path/slug, required sections, language consistency, diagram sync, brand residue). It does **not** write into `docs/` here: a plan at planning time is not yet implemented, so there is no durable knowledge to extract — writing it to `docs/` would pollute the reader-facing layer with unbuilt, speculative content.
- **At finalize (`/gal finalize` Sequence 2 + 5) — the completed plan's durable knowledge lands in `docs/`. This is a hard rule.** The steward extracts the *implemented* plan's knowledge into the durable layer (`README.md` + `docs/`), then the index is re-synced into `.dev/project.md`. Landing is enforced, not advisory: **ORCHESTRATOR may not delete the plan files until the steward has produced the durable-layer commit hash** (Sequence 5 pre-deletion gate), and `gal finalize-check`'s `durable-layer-commit` check asserts that commit exists. A plan cannot be closed with its knowledge unwritten.

Why the split: knowledge extraction requires *built* knowledge, which only exists after implementation — so the "write the finished plan into proper docs" rule belongs at finalize (end of lifecycle), while planning-time steward keeps the plan document itself well-formed.

**Planning loop.** The `/planning → /deep-planning → /refining-plan` flow is a single REFINE-LOCK loop (atomicity rubric → Definition-of-Ready dual lens → convergence, iterating to a conjunction exit). `/plan-to-prompt` comes only after that loop has converged and human approval is recorded in `## Approval`. The canonical flowchart is in [plugins/gal-core/workflows/coding.md](../plugins/gal-core/workflows/coding.md#stage-35--definition-of-ready-gate-refine-lock-loop) — see it there rather than a duplicate copy.

### Role Invocability Matrix

| Role | Directly callable? | Mode(s) |
| --- | --- | --- |
| **architect** | Yes — consult | `/gal architect` (isolated) · `/gal discuss architect` (in-context) |
| **analyst** | Yes — consult | `/gal analyst` (isolated) · `/gal discuss analyst` (in-context) |
| **designer** | Yes — consult | `/gal designer` (isolated) · `/gal discuss designer` (in-context) |
| **releaser** | Yes — consult | `/gal releaser` (isolated) · `/gal discuss releaser` (in-context); planning designer, does not execute |
| **debugger** | Yes — utility | `/gal debugger` |
| **steward** | Yes — utility | `/gal steward` |
| **implementer** | **Orchestrated-only** | Only via `/gal pipeline` (pipeline-phase context) |
| **tester** | **Orchestrated-only** | Only via `/gal pipeline` (pipeline-phase context) |
| **auditor** | **Orchestrated-only** | Via `/gal pipeline` (pipeline-phase) or `/gal finalize` (branch-audit) |
| **researcher** | **Orchestrated-only** | Only via `/gal research` or `/gal deep-research` |

A bare `/gal <role>` without the matching orchestration context returns **unknown-intent** for the four orchestrated-only roles — they remain registered in `KNOWN_GOLEMS` (pipeline dispatch still resolves them) but cannot be invoked interactively.

### Consult Dual-Mode (`/gal discuss <role>`)

Planning-stage consult roles (architect, analyst, designer, releaser) support two invocation modes:

| Mode | Trigger | What happens | Response label |
| --- | --- | --- | --- |
| **Isolated** (default) | `/gal <role>` | Native subagent runs role in isolation; only verdict/summary returns to main context | `[<role> · isolated]` |
| **In-context** | `/gal discuss <role>` | Activation-core (the `<role>…</role>` section) loads into the current conversation; the assistant hot-joins from any prior isolated verdict and continues multi-turn until the topic changes | `[<role> · in-context]` |

**Hot-join**: the isolated verdict is already in the transcript, so in-context mode continues from where it left off without re-running the role from scratch.

On Codex, `/gal discuss <role>` maps to `$discuss-<role>` (the shared `~/.agents/skills/discuss-<role>/SKILL.md` is installed via `gal init`). On Claude, use `/gal discuss <role>` as a slash command in the Claude interface.

The shared [`adversarial-review`](../plugins/gal-core/skills/adversarial-review/SKILL.md) skill is the target-agnostic review method used by consult review roles. Apply it when you want steel-man, refute-by-default, evidence discipline, explicit `APPROVE`/`REVISE`/`REJECT` verdicts, jidoka stop-lines, and `NotRun`≠pass on any plan, diff, doc, or decision. Because the pointer lives in each consult role's `<role>` core, `/gal discuss architect|analyst|designer` carries the method in-context automatically.

For the authoritative invocability contract and role-gate semantics, see [plugins/gal-core/workflows/coding.md § Direct Agent Invocation](../plugins/gal-core/workflows/coding.md#direct-agent-invocation).

### Release-Plan Lane

A release is its own plan type (`release-<slug>`), not a phase of `/gal pipeline` and not a step inside `/gal finalize`.

**Flow:**

1. `/gal releaser` — consult: reads project context, researches APIs/CICD tools, designs the release/devops flow, emits design advice. Read-only — does not write files, does not commit, does not execute.
2. `/planning release-<slug>` — materializes the advice into a source plan with `## Tasks`.
3. `/gal pipeline` — normal pipeline (implementer/tester/auditor) executes the release plan.
4. `/gal finalize` — lands the release plan like any other plan (doc-sync + lifecycle close).

`/gal finalize`'s Sequence 2 is doc-sync only — it no longer contains a release step.

### Deploy Capability Resolution (ask-first + four surfaces, read-only)

`golem-releaser` always confirms the deploy target with the user first (**always-ask-first**, regardless of whether project context could infer it), then resolves locally-available deploy capabilities across four surfaces before designing the flow:

| Surface | What releaser reads |
| --- | --- |
| **MCP** | `~/.gal/generated/mcp/managed.json` + host MCP configs (`~/.claude.json` `mcpServers`, `~/.codex/config.toml` `[mcp_servers]`, other selected runtimes) — filtered by relevance to the confirmed target |
| **skills** | Canonical root (`~/.gal/plugins/gal/skills/`) + host-specific skills dirs (e.g. `~/.copilot/skills/`, `~/.agents/skills/`) |
| **API** | Web-research for deploy/CICD API options appropriate to the confirmed target platform |
| **CLI** | Config files that record tool status (e.g. `.firebaserc`, `firebase.json`, `docker-compose.yml`); tools not confirmable by file read are clarified in the Step 0 user exchange |

**Honest reporting rule:** a read-miss or absent path means the capability is **not available** — releaser reports it explicitly. No fabrication. No PATH execution-probe (read-only constraint preserved).

**Scope:** `golem-releaser` supports both GAL-self CLI binary releases (GitHub Releases + package managers) and downstream repo deploys (web service, backend service, firebase-class, npm package, Docker image, etc.), adaptive to the user-confirmed deploy target. Scope stops at target-driven **design advice** — releaser is not a deploy orchestrator (no canary/rollback/production monitoring).

## Named Workflow Obedience

GAL named workflows (`$gal-pipeline`, `$gal-finalize`, `$gal-status`, `$deep-planning`, `$refining-plan`, `$plan-to-prompt`) require the active runtime to load and execute the corresponding `SKILL.md` **before** taking any other action. Generic autonomous coding or batch edits that bypass a named workflow is a **named workflow obedience failure**.

### Enforcement Surfaces

Two always-on surfaces carry the obedience rule:

1. **AGENTS.md preamble** (`## Adapter Rules`) — the Named Workflow Obedience bullet is rendered by `crates/cli/src/gal/render.rs` as the **first entry** in the preamble array. It appears within the first ~4 KiB of AGENTS.md and well within the 32 KiB Codex context read limit. A unit test (`agents_md_contains_obedience_marker_within_byte_limit`) asserts this offset on every build.
2. **Command-skill frontmatter descriptions** — each named-workflow command's description front-loads the explicit `$skill-name` trigger + key EN/ZH activation terms in the first ~200 chars so that description-based skill routing can resolve on partial reads.

### Explicit vs. Implicit Routing

- **Explicit invocation** (`$gal-pipeline`, `/gal pipeline`): the runtime sees the exact token and must load the corresponding SKILL. AGENTS preamble reinforces this for Codex.
- **Implicit invocation** (user says "implement this", "跑 pipeline"): depends on description-based skill matching. Trigger term front-loading improves salience but does **not** guarantee match — this is best-effort on all runtimes.

### Pipeline Entry Latch

`gal-pipeline` has a binding **Entry Latch** (rendered in `## Entry Latch` before `## When to Use`):

- Before any implementation edit, run `gal pipeline-preflight <prompt> --receipt <path>`.
- Only `overall: pass` allows proceeding; `fail` or `not-run` → STOP immediately, no file edits.
- Multiple plan references in one request must be expanded to an **ordered run list** and executed sequentially (never interleaved).

### Honesty Boundary

- AGENTS.md rules improve salience but cannot enforce compliance from a non-cooperative runtime.
- **Codex as a bounded executor fallback**: if Codex repeatedly ignores named workflows in interactive orchestration, route it via `gal pipeline` headless dispatch (`gal dispatch` + `config.json#executorRouting`) as CODER/TESTER rather than interactive orchestrator. This is documented in `docs/architecture.md` D17.
- A live probe showing skill load (`$gal-pipeline` loaded + preflight receipt exists) is signal; absence is not proof of permanent failure — routing depends on model, context window, and token budget.

## Responsibility Boundary

- Methodology and durable contracts stay in tracked repo files.
- Machine-local values stay in `*.local.*` files or runtime-owned config.
- Collaborative-tool availability is machine-local setup; readiness is workflow preflight via [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md).
- If a setting would create cross-machine drift, first ask whether it belongs in a source file instead of a local override.

## Read Next

- [../README.md](../README.md) — the map and what GAL is.
- [devguide.md](devguide.md) — maintainer-facing architecture, distribution, and runtime topology.
