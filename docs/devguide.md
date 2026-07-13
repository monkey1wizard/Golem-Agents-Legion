# Developer Guide

This document is maintainer navigation, not a second specification. Use it to decide which layer you are changing, which source files own that layer, and which rules you must not break.

## Overview & Dev Setup

**Contributor onboarding vs. maintainer reference:** [`contributing.md`](contributing.md) is the thin entry point for contributors — branching, build/test gates, commit conventions, and the 5-step workflow skeleton. This guide (`devguide.md`) is the deep maintainer reference: layer navigation, config schemas, protected-path rules, and runtime topology. Start with `contributing.md`; come here when you need depth.

To set up GAL for working on GAL itself, see the **[contributor setup in the README](../README.md#working-on-gal)**. This guide assumes you already have a source checkout; the `gal` binary resolves the source root automatically (cwd-walk → packaged layout), with no mode toggle.

If you cannot tell which layer you are touching, stop and resolve that first. Most broken refactors in GAL come from mixing README, docs, templates, scripts, and command contracts in one change. The `Codebase & Runtime Structure` section below is the map; the `Making Changes` section is the procedure.

## Source-Root Resolution

GAL uses a single operational topology: the `gal` binary locates its source root via a `.git`-bounded cwd-walk, then falls through to a binary-side packaged layout. There is no `devMode`/`galRoot` toggle — the source root is resolved automatically from the environment.

Key rules:

- Configuration lives in `~/.gal/config/config.json`.
- Remote (SSH) execution is a per-role `executorRouting` field (`sshTarget`/`remoteWorkdir`), not a separate config file — see `docs/remote-execution.md`.
- `GAL_SKILLS` is no longer part of the config surface.
- `context7ApiKey`, once rendered into `~/.gal/generated/mcp/managed.json`, is machine-local secret-bearing state — never tracked or shared.

### Dev Inner Loop (edit → see it live)

A source edit in `plugins/gal-core/` does **not** reach the coding agents on its own. Nothing auto-deploys after a commit. To make a change actually run:

| You changed... | Steps to see it live |
| --- | --- |
| `plugins/gal-core/` content only (SKILL/agent/command/conventions) | `gal refresh --source ./plugins/gal-core` |
| `crates/` (any Rust) | rebuild (`cargo build --release -p gal-cli`) → copy the binary onto `PATH` (`cp target/release/gal.exe ~/.cargo/bin/gal.exe`) → `gal refresh --source ./plugins/gal-core` |

`gal refresh` re-renders the canonical root and re-projects all five runtimes; the new SKILLs reference whatever subcommands the binary exposes, so a stale `PATH` binary makes the projected SKILL call commands that return `unknown command`. Always update the binary **before** refreshing when `crates/` changed. **Claude alone needs a restart** to reload; the other four runtimes pick up the refreshed skills on next invocation.

`gal refresh` accepts `--source <path>` to point at an explicit source root. On a dev checkout, always pass `--source ./plugins/gal-core` — without it, `gal refresh` tries to locate source beside the binary (packaged flat layout), which won't work from `~/.cargo/bin`.

**Two recurring traps:**

- **Missing `--source` on a dev checkout.** Running bare `gal refresh` from `~/.cargo/bin` fails to locate source because there are no `skills/`+`agents/`+`commands/` dirs beside that binary. Always pass `--source ./plugins/gal-core` (or `--source=./plugins/gal-core`) when running from a dev checkout.
- **`preserved user-owned path` warnings = a skipped projection.** Pre-lockfile stale projections (no GAL header, not in the attribution lockfile) are protected by the projection fail-safe and won't be overwritten. Confirm the named dir is GAL content (frontmatter `name: gal-*` / a known command), then remove it and re-run `gal refresh --source ./plugins/gal-core` (the second run reports 0 warnings).

**Codex freshness diagnostics (catch a missed inner-loop step).** After a source-contract change that needs `gal refresh`, `gal doctor` now catches the two ways a Codex session can run a stale contract — so you do not have to notice it by hand:

- **Stale skill projection** — a projected `~/.agents/skills/<name>/SKILL.md` (skill *or* command-projected skill like `gal-pipeline`) whose content differs from the current canonical source (`plugins/gal-core/skills/<name>` or `.../commands/<name>`) is reported stale → run `gal refresh` (then restart Codex / open a fresh thread). This means a forgotten refresh after editing a SKILL template is now a visible doctor finding.
- **Binary/source skew** — when the `gal` binary on PATH was built from an older commit than the GAL-checkout HEAD (baked `GAL_GIT_STAMP` ≠ `git rev-parse --short HEAD`), doctor tells you to rebuild + reinstall. This is the "I edited `crates/` but forgot to reinstall the binary" trap made visible. It is distinct from a stale skill: a fresh skill projected by a stale binary still calls the binary's older subcommands.

Rule of thumb for Codex: **`plugins/gal-core/` change → `gal refresh`; `crates/` change → rebuild + reinstall the binary *then* `gal refresh`.** Both then need a Codex restart / fresh thread to reload.

There is no binary-vs-source version stamp yet — `gal --version` is a static `0.1.0`, so it cannot tell you whether the `PATH` binary was built from your current source. Until a build-time git stamp lands, rebuild-and-refresh whenever in doubt.

## Codebase & Runtime Structure

This section is the single map of where everything lives. The two annotated trees below replace the old separate "find the right layer", "where information belongs", and "owning surfaces" sections — each node carries its purpose, layer, and owner. For the **crate dependency-law DAG and per-crate roles**, see [`architecture.md`](architecture.md) (the structure authority — not duplicated here).

### Codebase Data Structure

```text
Golem-Agents-Legion/
├── plugins/
│   └── gal-core/                canonical source-contract root
│       ├── commands/            public /gal command surface · source contract · changing it affects every runtime   [protected]
│       ├── conventions/         portable rules all golems follow · source contract                                   [protected]
│       ├── workflows/           workflow contracts (coding, doc-sync, research) · source contract                   [protected]
│       ├── templates/           durable repo-state templates · changing them reshapes every initialized repo        [protected]
│       ├── agents/              golem agent contracts (*.agent.md) + agents.md index · source contract
│       ├── skills/              skill bodies (skills/<name>/SKILL.md) · source contract
│       ├── mcp.json             tracked MCP manifest · source contract
│       └── opencode.json        tracked OpenCode runtime config shipped with gal-core
├── crates/          Rust workspace — the runtime (7 crates; DAG + per-crate roles → docs/architecture.md) · owns render/projection/dispatch/pipeline  [protected*]
├── packaging/       release packaging templates (homebrew/winget) + publish-public.sh (private→public mirror export); shell runtime ported to crates/ and deleted
├── docs/            documentation
│   ├── manual.md            user operations manual (canonical EN)
│   ├── devguide.md          this file — maintainer guide
│   ├── collaborative-tools/ one capability/tool contract per file (+ examples/)
│   ├── i18n/<lang>/         all translations (mirror layout, <name>.<lang>.md)
│   └── structure/           steward NDJSON structure map + schema
└── .dev/            repo working state
    ├── project.md           compressed project summary + Source Documents index
    ├── state.md             active plans index + session continuity
    ├── plans/<slug>.md       source plans (human-readable, transient) + <slug>.prompt.md execution work files (mutable task memory)
    └── research/            research work files (non-durable; findings promote into docs/)
```

\* Protected paths: `crates/projection/`, `crates/gal-engine/src/render/`, and the `plugins/gal-core/` workflow contracts. Touching any protected path requires an architect-reviewed plan before implementation.

### Where Information Belongs

| Information type | Right home |
| --- | --- |
| durable methodology and contracts | tracked source docs and source files |
| repo working context | `.dev/project.md` and `.dev/state.md` in the target repo |
| human-readable feature plan | `.dev/plans/<plan-slug>.md` |
| machine-readable execution work file | `.dev/plans/<plan-slug>.prompt.md` |
| temporary session continuity | `### Handoff Notes` plus `.dev/state.md` |
| machine-local remote-execution routing | `~/.gal/config/config.json#executorRouting` (`sshTarget`/`remoteWorkdir`) |

If a completed plan contains knowledge that should survive, extract it back into a durable source file instead of leaving the plan as hidden long-term documentation.

### Owning Surfaces (which source files own what)

| If you are changing... | Ask first... | Read these source files |
| --- | --- | --- |
| `/gal` command surface, aliases, or dispatch | is this control-plane behavior or runtime plumbing? | [../plugins/gal-core/commands/commands.md](../plugins/gal-core/commands/commands.md), [Former Scripts → Owning Crate](#former-scripts--owning-crate) |
| planning flow or optional collaborative-tool semantics | is this GAL-native planning, optional gstack behavior, or workflow teaching? | [../plugins/gal-core/commands/commands.md](../plugins/gal-core/commands/commands.md), [collaborative-tools/gstack.md](collaborative-tools/gstack.md), [../plugins/gal-core/workflows/coding.md](../plugins/gal-core/workflows/coding.md) |
| install topology, baked command files, or MCP merge | is this machine-layer install or repo-layer adapter generation? | [Former Scripts → Owning Crate](#former-scripts--owning-crate), `crates/gal-engine` (render/install), `crates/cli/src/main.rs` |
| templates and plan lifecycle | which file should own this information? | [../plugins/gal-core/templates/templates.md](../plugins/gal-core/templates/templates.md), [../plugins/gal-core/workflows/coding.md](../plugins/gal-core/workflows/coding.md) |
| user-facing install copy | who owns the words users read? | [getting-started.md](getting-started.md), [../README.md](../README.md), [manual.md](manual.md) |
| source-root resolution (cwd-walk/packaged) | which doc owns the user's machine intent? | [manual.md](manual.md), this guide |
| release / channel lineage | where is the release contract? | `release-<slug>` plan type (see `docs/architecture.md` D8); `golem-releaser` consult designs it; `gal release` binary subcommand generates artifacts (`CommandKind::Release`); `.github/workflows/release.yml` is the CI trigger |
| remote (SSH) execution behavior | is this part of the main dispatch path or a separate extension? | it is a spawn lane inside `crates/dispatch` (`docs/architecture.md` D22), not a separate crate — see `docs/remote-execution.md` |
| non-English planning language authority, hash/parity gates | is this deterministic (Rust) or semantic (planning-stage command)? | `crates/cli/src/commands/planning_authority.rs` (shared helper), `planning_check.rs`, `prompt_check.rs`; `docs/architecture.md` D20 |

### Planning-Language Authority (helper + checks + inline equivalence stamp)

For `planLanguage != en`, the deterministic layer lives in **one shared helper module** `crates/cli/src/commands/planning_authority.rs` so metadata parse, rendered-source hashing, and equivalence stamping are never triplicated:

- `parse_localized_meta` / `LocalizedMeta` — the localized-source metadata block schema (six fields: `semantic-draft`, `planLanguage`, `draft-hash`, `rendered-source-hash`, `prompt-hash`, `equivalence-verdict`). All six must be non-empty or the block fails to parse; `prompt-hash: none` / `equivalence-verdict: pending` are the valid pre-prompt placeholders.
- `rendered_source_hash` — excises the metadata block (no self-reference), normalizes LF + Unicode NFC + trailing-space strip, then SHA-256. Cross-machine stable (Windows CRLF == LF).
- `stamp_equivalence` — derives the paired source plan from a prompt path and overwrites its `prompt-hash` + `equivalence-verdict` fields in place. There is no receipt reader — the fields live only in the source plan.

Consumers:

- **`planning-check`** (`planning_check.rs`) — reads the localized metadata, validates rendered-source hash freshness + EN-draft presence/hash, checks machine-anchor parity (IDs + file paths, never prose), the zh-TW language gate (pure-English-line fraction, not Latin-letter ratio), and whole-line (non-code-span) marker detection. Never requires `equivalence-verdict: EQUIVALENT` — a pending pre-prompt plan still passes.
- **`prompt-check`** (`prompt_check.rs`) — for a non-English prompt whose source used the EN-draft flow, requires the source plan's `equivalence-verdict` to be `EQUIVALENT` AND its `prompt-hash` to match a live re-hash of the prompt body; whole-line anchor detection.
- The write-side (`gal planning-stamp <localized-source>` / `gal planning-stamp --equivalence <prompt>`, internal subcommand) writes the metadata hashes and the inline equivalence stamp deterministically — the model never hand-computes a hash. `--equivalence` mode takes no `--receipt` argument.

**No sibling receipt file.** The equivalence proof lives entirely inside the source plan's `gal:planning-authority` block — there is no `.dev/plans/<slug>.equiv.md`. A non-English plan has exactly two tracked files after `/plan-to-prompt` (`.md` + `.prompt.md`). The three plan-token classifiers (`commit_msg.rs`, `dispatch_script/model.rs`, `commands/dispatch.rs`) still exclude a stray `.equiv.md` suffix as a defensive guard for a legacy or downstream orphan file — nothing writes one anymore. The semantic merge of a localized edit back into the EN draft stays in the planning-stage command (model), never in Rust.

### Former Scripts → Owning Crate

The PowerShell/Bash automation that used to live under `scripts/` was fully ported to the `gal` Rust binary and deleted (scripts→Rust migration, `cfb82af`). Use this table to find the owning crate:

| Former script behavior | Now owned by |
| --- | --- |
| `gal <subcommand>` dispatch (`gal.ps1`/`gal.sh`) | `crates/cli` (`gal` binary; chat-control-plane dispatch = `gal dispatch-script`) |
| Shared path/runtime constants (`common.ps1`/`common.sh`) | `crates/gal-foundation` (`platform`, path resolution) |
| git-filter registration (manual `git config filter.*`) | `crates/gal-engine` (`git_filter`, `crates/cli` `cmd_filter_transform`) |
| MCP merge (`Update-Mcp.ps1`/`update-mcp.sh`) | `crates/mcp` + `crates/gal-engine` render (`src/render/`) |
| Repo-local adapter + baked-skill generation | `crates/projection` (`gal init`) |
| Canonical-root plugin render | `crates/gal-engine` (`src/render/`) |
| xmachine remote/local task lanes (`Start-xMachine.*`, `Invoke-Xmachine*`, `Get-Xmachine*`) | retired (`refactor-ssh-transport-replace-xmachine`) — the `crates/xmachine` crate and `gal xmachine` subcommand are gone; remote execution is now a spawn lane inside `crates/dispatch` (see D22) |
| Task-spec materialization (`New-TaskSpec.ps1`) | `crates/pipeline` (`task_spec`) |
| Install acceptance (`test-install-acceptance.sh`) | `gal doctor` + real-machine acceptance plan |

The only non-runtime shell kept is `packaging/publish-public.sh` (curated private→public mirror export; ops-only, not a runtime command). Release packaging templates live under top-level `packaging/`.

### Language Conventions & Personal Layer

`plugins/gal-core/conventions/` has **two independent consumers** that must both be kept in mind before adding anything to the directory:

1. **The selector** (`project_convention_file_names` + `personal_convention_docs` + `detected_language_skills_block`, all in `crates/cli/src/gal/render.rs`) — picks a specific file set per repo based on the `Language` row.
2. **The instruction corpus sweep** (`render_instruction_corpus`, `crates/gal-engine/src/render/manifests.rs`) — copies **every** `.md` file in the directory into the canonical instruction corpus, unconditionally, with no selector logic.

Because of consumer 2, **no example or sample file may live inside `plugins/gal-core/conventions/`** — it would be swept into the corpus regardless of Language matching. The one shipped example (`csharp-convention.example.md`) lives at `plugins/gal-core/templates/` instead, which neither consumer reads.

**Dual-source selection + token matching:** the selector reads the project's `| Language |` row from `.dev/project.md`, tokenizes it with `language_tokens` (lowercase, split on non-token chars; token chars = alphanumerics + `#`/`.`/`+`, so `c#`/`.net` survive as single tokens), and matches word-boundary — not substring — so `Django`/`MongoDB` never false-match `go`. `rust.md` selects only on an explicit `rust` token; the no-Language fallback is the neutral three (`conventions.md`/`token-budget.md`/`working-hours.md`), excluding `rust.md`. Personal convention files (`~/.gal/local/conventions/<lang>.md`) reuse the same tokenizer, with a small alias table (`c#`/`.net`→`csharp`, `javascript`→`typescript`, `golang`→`go`); a stem outside that alias table is treated as generic/universal and always matches.

**Detected-skill scanner** (`crates/cli/src/gal/skill_discovery.rs`) enumerates four skill roots: the Claude plugin cache (expanded per-plugin via `claude_plugin_skill_roots` — every installed plugin, not just `cache/gal`), the shared agents skills root (`~/.agents/skills`), Copilot's skills root, and agy's skills root. It parses each `SKILL.md`'s YAML frontmatter for `name`/`description` (a minimal hand-rolled parser, no YAML dependency) and returns a sorted list; missing/unreadable roots and malformed entries are skipped without panicking. The render layer matches only a skill's **name + origin path** tokens against the Language line — descriptions are display-only and never matched (a false-positive generator for short tokens like `go`) — and rejects any name failing a safe charset (alphanumerics, `-`, `_`, bounded length) before it can reach a rendered instruction block.

**Personal-layer gates are presence-based**, not flag-gated: `crates/gal-engine/src/render/mod.rs`'s two gates (personal skills merge, personal MCP merge) and `crates/gal-engine/src/doctor.rs`'s `check_personal_layer` finding all key off directory/file existence under `~/.gal/local/` only — there is no `personalLayer.enabled` config key. Placing a file at the personal root is itself the opt-in; an absent path keeps every render byte-identical to a Core-only render.

### .gal Data Structure

```text
~/.gal/                          machine-local runtime root (never the source of truth)
├── config/                      USER-OWNED machine intent — preserved across upgrade/uninstall
│   └── config.json              machine config: personalization, secrets, profiles, executorRouting (incl. sshTarget/remoteWorkdir)
├── state/
│   └── plugins.lock.json        MIXED-OWNERSHIP: resolver keys (GAL-written) + `_galProjection` (projection registry, GAL-written)
├── plugins/gal/                 GAL-MANAGED canonical plugin root — the runtime content owner
├── generated/                   GAL-PRODUCED projections, rebuildable
│   ├── mcp/managed.json         resolved MCP (SECRET-BEARING, machine-local, never shared)
│   └── providers/               provider projections
└── active/<provider>/           stable shortcut targets for AI tools (consumers never point at store paths)
```

Provider-visible projections live outside `~/.gal/` (e.g. `~/.claude/skills/gal`, `~/.copilot/skills/` + `~/.copilot/agents/`, `~/.gemini/antigravity-cli/skills/`) and are aliases of the canonical root, not second sources of truth.

The four authoritative runtime files have explicit schemas below.

### Per-Runtime Command Loading (projection matrix)

GAL authors every control-plane action as a **command** (`/gal status`), but only Claude Code and Gemini have a real slash-command concept. Copilot, Codex, and Antigravity load capabilities as **skills**. The projection layer (`crates/projection/src/lib.rs` `update_commands`) adapts each command per runtime — `crates/gal-engine/src/render/` is **not** involved (canonical root keeps `commands/` as source of truth):

| Runtime | Command-loading mechanism | Projection target | Trigger |
| --- | --- | --- | --- |
| Claude Code | native plugin commands | canonical-root `commands/` | `/gal status` |
| Gemini | native TOML slash command | `~/.gemini/commands/<name>.toml` | `/gal-status` |
| Copilot | **skill** (no user slash command) | `~/.copilot/skills/<name>/SKILL.md` | `/gal-status` |
| Codex | **skill** (custom prompts deprecated) | `~/.agents/skills/<name>/SKILL.md` (official) | `$gal-status` |
| Antigravity | **skill** (no native `commands/` folder — gemini-cli#27325; commands are Agent Skills) | `~/.gemini/antigravity-cli/skills/<name>/SKILL.md` | `/gal-status` |
| opencode | native command | `~/.config/opencode/commands/<name>.md` | `/gal-status` |

**Double-load split:** a runtime with a native command mechanism (Gemini, opencode, Claude) does **not** also receive a command-skill. The shared `~/.agents/skills` is read by codex+opencode+copilot, so the command-skill there is written **only when codex is selected** — an opencode-only install keeps just its native command. `write_command_skill` preserves a genuine user directory at a colliding name (positive lockfile `is_managed` test, first-run safe). Deselecting a runtime removes its GAL-written command-skill (`remove_gal_command_skill`) so it cannot linger in the shared dir and double-load.

### Agent Projection Matrix

GAL projects golem agent files per runtime in addition to commands/skills. The projection format is **not** a verbatim copy — it adapts the `*.agent.md` source to the target runtime's native agent mechanism (per `docs/architecture.md` **D7**):

| Runtime | Agent format | Projection target | Mechanism |
| --- | --- | --- | --- |
| Claude Code | `*.agent.md` (verbatim with GAL header) | `~/.gal/plugins/gal/agents/` → Claude subagent | Claude native plugin subagents |
| Codex | `*.toml` (`[agent]` section) | `~/.codex/agents/<name>.toml` | Codex TOML subagent (`crates/projection/src/codex_agent.rs`) |
| Antigravity | None — consumes skills | `~/.gemini/antigravity-cli/skills/` | Agent surface covered by projected skills; native agent-file load unverified |
| Copilot / opencode | None — no native subagent concept | — | Not projected; skills cover the command surface |

**Discuss-skill projection (consult in-context mode):** when Codex is a selected runtime, `gal refresh` also writes `discuss-<role>/SKILL.md` to `~/.agents/skills/` for each of the four consult roles (`golem-architect`, `golem-analyst`, `golem-designer`, `golem-releaser`). These skills inject the role's `<role>…</role>` activation-core into the current Codex session without spawning a subagent. The discuss-dir names are added to the shared-skills prune keep-set so the shared `~/.agents/skills/` prune does not evict them.

**Orchestrated-only exclusion:** the roles `{implementer, tester, auditor, researcher}` are **excluded** from native subagent projection (`crates/projection/src/lib.rs` `update_agents`). They remain in `KNOWN_GOLEMS` (pipeline dispatch resolves them) but have no `*.agent.md` or `*.toml` projected file. This prevents bare interactive invocation — a user typing `/gal tester` or `$tester` in a Codex session receives an unknown-intent error rather than running an unorchestrated tester pass.

**Tool name mapping single source:** abstract permissions in frontmatter (`read/edit/execute/search/web`) are mapped to valid runtime tool names per runtime by `crates/projection/src/tool_map.rs`. Invocation mode (consult vs build) controls sandbox permissiveness: consult = `read-only` (Codex), build = `workspace-write`.

### Claude Plugin Cache Refresh (R6)

Claude Code copies the local directory-source plugin into a cache at `~/.claude/plugins/cache/gal/` and serves that copy. Because GAL's manifest carries **no** `version` (see `render_claude_plugin_manifest`), a stale cached copy would otherwise persist after the content changes. **Restart Claude Code** after updating `gal init` output to pick up new commands. Fully automatic refresh-on-commit (a git-source marketplace) is deferred to GAL's public release.

**When the cache is stale** (the canonical root was rebuilt but Claude still reads the older cache), run the following steps:

1. Run `gal doctor` — it reports a cache-staleness warning when `~/.claude/plugins/cache/gal/` is older than `~/.gal/plugins/gal/`.
2. Restart or reload Claude Code — it rebuilds the cache from the canonical root automatically on startup.
3. If the warning persists after restart, manually remove `~/.claude/plugins/cache/gal/` and restart Claude Code.

This situation is most common after updating the binary and re-running `gal init` in a running Claude Code session. The `ClaudePluginCacheCheck` in `crates/gal-engine/src/doctor.rs` emits a non-blocking `Warning`-severity finding when the mtime ordering indicates staleness.

### Runtime-Host Component Placement

The command→skill projection above covers Copilot/Codex with **no extra component** — GAL is skill-only today. **If** a future integration ever needs a dedicated runtime-host component (e.g. a VS Code extension that surfaces GAL commands in Copilot Chat), it is a runtime **consumer**, not part of the plugin, and is placed surface-namespaced:

| Layer | Location |
| --- | --- |
| Machine | `~/.gal/<surface>/…` (e.g. `~/.gal/vscode/extension/`) |
| Repo source | a parallel surface-namespaced directory |
| **Forbidden** | inside `~/.gal/plugins/gal/…` (canonical plugin root) or a repo-root `extensions/` |

The canonical plugin root holds only the projected plugin; a host that reads it lives beside it, not inside it. Full rationale: [`docs/architecture.md`](architecture.md) — **D5 (Runtime-host components are consumers, not part of the plugin)**.

### Runtime File Schemas

This subsection defines the structure, required fields, optional fields, secret boundaries, precedence rules, and drift metadata for the four authoritative `~/.gal/` runtime files.

#### `~/.gal/config/config.json` — User-Managed Machine Config

Consolidates machine settings: **GAL-core** routing fields (`defaultProfile`/`profiles`/`enabledPlugins`/`disabledPlugins`/`executorRouting`), retained machine-local path substitution via `galSkills`, working-hours preferences under `workingHours`, and a `secrets` map for credentials. The retired `devMode`/`galRoot` keys are silently ignored if present; `gal doctor` emits a non-blocking warning.

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `schemaVersion` | yes | integer | schema version for migration |
| `defaultProfile` | no | string | default profile name; defaults to `default` |
| `profiles` | no | object | map of profile name → plugin list |
| `enabledPlugins` | no | array | explicit plugin selections |
| `disabledPlugins` | no | array | explicit plugin exclusions |
| `updateChannel` | no | string | update policy |
| `allowAutoUpdate` | no | boolean | per-plugin auto-update override |
| `executorRouting` | no | object | role routing grouped by consumer: `pipeline` (dispatch `{CODER,TESTER,AUDITOR}`) + `planning` (consult `{ARCHITECT,ANALYST,DESIGNER,RELEASER}`) + shared `executors` default-model block; sole routing source. The flat shape (role keys directly under `executorRouting`) is retired — not parsed, warns by name (the standalone `executor-routing.json` is also no longer read) |
| `secrets` | no | object | UPPER_SNAKE keys → secret string values; **SECRET-BEARING** — do not share; values materialized into `mcp/managed.json` |
| `galSkills` | no | string | path to the machine-local skills directory; the only retained machine-path substitution key |
| `mcpFilesystemPaths` | no | array | optional compatibility field; only output when filesystem MCP is present |
| `workingHours` | no | object | working-hours enforcement config (`enabled`, `workdayStart`, `workdayEnd`, `wrapUpTime`, `hardStopTime`); all `HH:MM` strings except `enabled` (boolean) |

**Secret boundary**: `secrets` values are the only secret-bearing fields. Once materialized into `~/.gal/generated/mcp/managed.json`, that generated file becomes machine-local secret-bearing state — never commit or share it. The file itself is gitignored by GAL's shipped `.gitignore` entry.

**Precedence**: `config.json` is the user-facing input. Explicit `enabledPlugins`/`disabledPlugins` override profile-level settings.

**External-note backends** stay outside the Core config surface. Their portable contract lives in `docs/collaborative-tools/local-notes.md`; repo-owned research still defaults to `.dev/research/`, and temporary GAL scratch data should use repo-relative `.dev/tmp`.

#### `~/.gal/state/plugins.lock.json` — Projection Registry

Holds the **`_galProjection`** namespace (top-level key), written by `projection::persist`: the managed-artifact registry recording which paths gal projected and their source attribution. Used by prune-safety so `gal update` never deletes a non-gal-managed file. Deterministic and rebuildable from a re-render.

| `_galProjection` sub-field | Type | Notes |
| --- | --- | --- |
| `agentProjectionPaths` | array | gal-managed agent file paths |
| `commandProjectionPaths` | array | gal-managed command file paths |
| `skillProjectionPaths` | array | gal-managed skill file paths |
| `legacyProjectionPaths` | array | gal-managed legacy/adapter file paths |
| `sourceAttribution` | object | path → source-id map for multi-source projection |

> A third-party-plugin resolver namespace (catalog resolution) is a ccync concern and is not written by gal.

#### `~/.gal/config/config.json#executorRouting` — Remote (SSH) Execution Binding

Remote execution is bound per-role inside `executorRouting`, not a separate file — a role entry gains `sshTarget` + `remoteWorkdir` (both required together). See `docs/remote-execution.md` for the field contract and safety boundaries.

**Secret boundary**: `sshTarget`/`remoteWorkdir` values are machine-local (SSH targets and repo paths); do not share `config.json`.

#### `~/.gal/generated/mcp/managed.json` — GAL-Produced MCP Projection

Generated file owned by GAL. Rendered from `plugins/gal-core/mcp.json` with machine-local values (e.g. `context7ApiKey`) resolved from `~/.gal/config/config.json`.

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `schemaVersion` | yes | integer | generated schema version |
| `generatedAt` | yes | string | ISO 8601 generation timestamp |
| `generatedBy` | yes | string | tool version that produced this file |
| `mcpServers` | yes | object | resolved MCP server configurations |
| `inputs` | no | array | resolved prompt-backed inputs |
| `_metadata` | yes | object | generation metadata |

**Server entry shape** (per server):

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `command` | conditional | string | CLI command (absent when `serverUrl` present) |
| `args` | conditional | array | CLI args (absent when `serverUrl` present) |
| `serverUrl` | conditional | string | HTTP MCP endpoint (absent when `command` present) |
| `env` | no | object | environment variables (resolved, no placeholders) |
| `headers` | no | object | HTTP headers (resolved, no placeholders) |

**Secret boundary**: `context7ApiKey` is materialized directly into `headers.CONTEXT7_API_KEY` — no `${CONTEXT7_API_KEY}` placeholder remains. This file is **secret-bearing machine-local state**. It must never enter a tracked repo, shared lockfile, or team configuration. Mark it in `.gitignore`.

**Resolved values rules**:

- `mcpFilesystemPaths` appears only when filesystem MCP is present in the resolved set; otherwise omitted.
- No runtime placeholder resolution is required — all values are fully materialized.

#### Schema Precedence Chain

```text
plugins/gal-core/ (repo source: skills/commands/agents/mcp.json)
  ↓ rendered with
~/.gal/config/config.json (user-owned, machine-local preferences + secrets)
  ↓ into
~/.gal/plugins/gal/ (canonical root) ──projected──► each agent surface
  ↓ also produces
~/.gal/generated/mcp/managed.json (machine-local, secret-bearing, never shared)
~/.gal/state/plugins.lock.json#_galProjection (projection registry, rebuildable)
```

## Distribution

The `gal` binary is distributed via package managers (cargo / winget / Homebrew / curl). After the binary is installed (or reinstalled), run **`gal refresh`** to rebuild the canonical plugin root (`~/.gal/plugins/gal`), the parent Claude marketplace manifest (`~/.gal/plugins/.claude-plugin/marketplace.json`), and per-runtime projections (copilot/codex/agy/gemini/opencode) from source.

**`gal refresh` does not write `~/.claude/plugins/cache` or `installed_plugins.json`** (those are Claude-managed). After a refresh, Claude re-picks up the updated plugin via restart or re-add (`claude plugin add ~/.gal/plugins`). `gal doctor` reports a cache-staleness warning when the Claude cache is older than the canonical root.

**All-5-runtime projection (F2 current design):** `gal refresh` always projects to all 5 runtimes because there is no active writer for `install-state.json` post-`fix-cli-command-surface`. Selective per-runtime projection is a future enhancement pending an install-state.json writer.

`gal init` generates repo-local adapters (CLAUDE.md, AGENTS.md, etc.) from the gal-core templates. `gal update` prints current version and upgrade instructions. `gal doctor` checks the local setup. See *Codebase & Runtime Structure → Per-Runtime Command Loading / Agent Projection Matrix* for the per-agent mechanics.

Public release and package-manager distribution (`winget` / `homebrew` / GitHub Releases / curl) ships via `packaging/` and `.github/workflows/release.yml`.

## Source Restore

`gal restore [--yes]` reverts the GAL *source* repo to the last `/gal finalize`-verified known-good state (source + derived layers). It does not copy machine-local config across machines.

### Two-Layer Model

| Layer | What restore does |
| --- | --- |
| **Source** | `git reset --hard gal-last-good` + `git clean -fd` — worktree **exactly** equals the marker commit; tracked files added after the marker and all untracked files are removed (A-semantics). |
| **Derived** | Delegates `gal refresh` — rebuilds canonical root + runtime projections from the restored source. |

### `gal-last-good` Marker

`/gal finalize` writes a lightweight git tag `gal-last-good` to the landing commit (Sequence 6, non-fatal). `gal restore` reads that tag as the **sole baseline** — absent tag → fail-closed with an actionable message; no HEAD fallback, no guessing.

**Bootstrap:** the tag does not exist until the first `/gal finalize` after this feature lands. `gal restore` will refuse cleanly until then.

### Safety Net

Before any mutation, `gal restore` creates a `gal-restore-backup-<ts>` branch at the current HEAD. This is a **hard precondition** — if branch creation fails, the command aborts without touching the worktree. Nothing is ever silently lost.

### Confirmation Guard

Without `--yes`, the command prints the discard scope (uncommitted changes + commits ahead of the marker) and exits with a usage error. This is the intentional "dry-run" path.

## Making Changes

### Rules & Fences

#### 1. Markdown owns the durable contract

- Methodology, rules, and contracts live in tracked Markdown and source files.
- Generated adapters, baked command files, and runtime configs are outputs, not source inputs.

#### 2. `/gal` only solves control-plane problems

- `/gal` should not wrap a second copy of tester, auditor, designer, debugger, or releaser work. `golem-releaser` is a planning-stage consult role, not a pipeline execution role — it runs before implementation, not after.
- Execution-stage specialist behavior belongs in agents.
- Planning commands can run directly because they are still part of the public command surface.

#### 3. Repo-local state is the ownership boundary

- `.dev/`, `.dev/plans/`, `docs/designs/`, and similar repo-local files are the shared working state.
- Do not move GAL's core state back into user-global storage.

#### 4. Missing tools must not look like success

- GAL uses skill-level routing, not one repo-wide CLI-first or MCP-first rule.
- Each external-tool skill should define a preferred path, a fallback path, and a no-tool behavior.

#### 5. Do not optimize one runtime by breaking portability

- If a change makes Copilot, Antigravity, and Codex diverge in contract or file flow, it is usually the wrong change.
- README, docs, templates, and setup scripts should preserve cross-runtime parity first.

#### 6. Navigation docs must not become a second spec

- If a document exists to help humans find the real source, keep it short and directional.
- Summaries are useful. Duplicate contracts are not.

#### 7. Collaborative-tool routing belongs to the workflow layer

- Do not make an agent silently switch personas or contracts because a collaborative tool was detected.
- The workflow decides the collaborative tool first, then the tool writes back into the same repo-owned files.

The shared preflight model lives in [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md).

### Before Editing Install

1. Decide which concern is changing: bootstrap payload, machine setup, provider plugin lifecycle, repo init, generated adapters, or documentation copy.
2. Read the primary owner from the `Owning Surfaces` table above, then read only the directly called scripts or docs for that concern.
3. For contributor builds, the source root is resolved via cwd-walk — no config change needed.
4. Preserve user-owned machine intent during upgrade and default uninstall.
5. If the change affects public install wording, update English and the translated copies under `docs/i18n/<lang>/` together, or explicitly mark the translation drift.
6. Run the narrowest install tests that cover the changed lane: `cargo test --workspace` + `gal doctor`.

### Common Change Entry Points

#### Changing `/gal` or alias behavior

1. Read [../plugins/gal-core/commands/commands.md](../plugins/gal-core/commands/commands.md).
2. Check whether the change is contract-level behavior or only install/runtime presentation.
3. If it affects generated command files, inspect the setup scripts and the relevant `commands/*/SKILL.template.md`.

#### Adding or changing a planning command

1. Place it in the right family via [../plugins/gal-core/commands/commands.md](../plugins/gal-core/commands/commands.md).
2. Update the owning prompt in `commands/<command>/SKILL.template.md`.
3. Confirm the write-back target fits the existing plan sections and workflow state machine.
4. If it changes optional collaborative-tool semantics, also update [collaborative-tools/gstack.md](collaborative-tools/gstack.md) and [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md) when shared preflight behavior changes.

#### Adding or changing an execution specialist

1. Update the owning prompt in `agent/<golem>.agent.md`.
2. Confirm the write-back target fits the existing plan sections and workflow lifecycle.
3. Update [../plugins/gal-core/agents/agents.md](../plugins/gal-core/agents/agents.md), [../plugins/gal-core/commands/commands.md](../plugins/gal-core/commands/commands.md), and any README sections that route users to that specialist.
4. Do not reintroduce the behavior as a standalone public command unless it is truly control-plane or planning work.

#### `/gal finalize` — plan completion landing (vs `/gal wrap-up`)

`/gal finalize` is the control-plane command that **lands and closes a completed plan**. It runs only after `/gal pipeline` has finished every task and the orchestrator's end-of-run goal-backward verification returned VERIFIED. It is a thin orchestrator with zero new authority: a hard precondition gate, then whole-branch holistic review (delegated to `golem-architect` for correctness/architecture/scope and `golem-auditor` standalone branch-audit for perf/security), then — on a worktree/feature branch — a plain `git merge` to main plus `git worktree remove`/`git branch -d` teardown, then release/doc-sync (STEWARD knowledge → `docs/`, release conditional + graceful degrade), then delegated lifecycle close (ORCHESTRATOR marks ABSORBED and deletes the plan files). The canonical contract lives in [../plugins/gal-core/workflows/coding.md](../plugins/gal-core/workflows/coding.md) → Plan Finalization; this guide does not redraw the flowchart.

**finalize vs wrap-up — keep these distinct:**

| | `/gal finalize` | `/gal wrap-up` |
| --- | --- | --- |
| Purpose | Completion **landing** (land + close a done plan) | Session **pause** (hand off mid-flight work) |
| When | All tasks `[x]` + goal-backward VERIFIED | Anytime, at any plan state |
| Effect | One-shot, gated, **destructive** (merge + plan-file deletion) | **Non-destructive** (handoff notes + session continuity) |
| Reverse-prompt | Offers `/gal wrap-up` if the user is only pausing | Offers `/gal finalize` once when the plan is complete |

A maintainer changing finalize behavior edits `commands/gal-finalize/SKILL.template.md` (the command contract) and `workflows/coding.md` (the canonical flow + delegation boundary), then re-syncs per the closeout below.

#### Closeout: re-syncing a roster/role contract change to a live runtime

When a change adds, removes, or renames a golem persona or a dispatch routing role, the source edit in `plugins/gal-core/` does **not** reach the user's live `/gal` surface on its own — the running runtime still loads the previously-installed plugin payload (and, on Claude Code, a possibly-stale versioned cache at `~/.claude/plugins/cache/gal/gal/<ver>/`). This is a **manual, machine-mutating closeout/release step**, run after merge by the maintainer or `golem-releaser`; it is **not** an automated `/gal pipeline` task (the pipeline must never machine-mutate a user's runtime install).

Procedure:

1. **Project source to the runtime:** run **`gal refresh`** to rebuild the canonical plugin root (`~/.gal/plugins/gal`) and re-project every runtime's skill/command surface (Claude, Codex, Copilot, Antigravity, opencode). This is the step that actually carries a `plugins/gal-core/` source edit to the live agents — `gal init --force` (step 2) does **not**: it only rebuilds repo-local adapters. Skipping `gal refresh` is the classic "I changed the source but the agents still run the old version" trap. See [Dev Inner Loop](#dev-inner-loop-edit--see-it-live) for the binary-staleness and staged-layout caveats.
2. **Regenerate repo-local adapters:** run `gal init --force` in each initialized repo to rebuild CLAUDE.md, AGENTS.md, and the other adapter files from the merged source.
3. **Migrate live machine-local config:** if a routing role was renamed, or the config still uses the retired flat shape, update the `executorRouting` subtree inside `~/.gal/config/config.json` to the two-group form (`pipeline` / `planning`). A retired flat role key degrades gracefully but **loudly** — `gal dispatch` prints a retirement warning naming the key and its target group (and `gal doctor` surfaces it), then falls back to text/in-process dispatch until the key is moved into the correct group. There is no alias or silent fallback.
4. **Refresh the Claude plugin cache:** restart Claude Code to pick up changes; the cache rebuilds from the plugin root automatically on startup. The other four runtimes pick up the refreshed skills on their next invocation — only Claude needs a restart.

Acceptance checklist (run on the target machine after re-sync):

- a removed/renamed golem no longer resolves — e.g. `/gal old-golem` no longer resolves.
- the live agents surface contains no removed persona file.

#### Changing installation, MCP, or git-filter

1. Read [Former Scripts → Owning Crate](#former-scripts--owning-crate).
2. Decide which concern owns the change: `gal init` (repo-local adapter render), `crates/gal-engine/src/doctor.rs` (health checks), or `crates/gal-engine/src/git_filter.rs` (git smudge/clean filter commands).
3. Windows and macOS/Linux share the single Rust render path; keep behavior aligned unless the change is intentionally platform-specific.
4. Check whether `commands/commands.md` should also change because the user-visible runtime surface changed.
5. Keep README focused on entry points, keep render plumbing here.

#### Regenerating Adapters and MCP

- After changing `plugins/gal-core/mcp.json`, run `gal refresh` to re-render the canonical root and runtime MCP projection; machine-local secret values still come from `~/.gal/config/config.json`. (`gal install`/`gal update` no longer project — `gal update` is a version-print stub; `gal refresh` is the projection path. See [Dev Inner Loop](#dev-inner-loop-edit--see-it-live).)
- Run `gal init --force` after changing source-of-truth content that should regenerate repo-local adapters such as `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, or `GEMINI.md`. Repo adapters and runtime projection are separate: `init --force` does adapters, `gal refresh` does the runtime skill/command surface.

#### Refactoring docs themselves

1. Make sure each doc has one clear job.
2. If another source file already owns the contract, summarize it and link out instead of copying it.
3. If you remove content from one reader entry point, give it a clear new landing page.

### Adding A New CLI Runtime

1. Decide whether the CLI has a machine-layer config directory that GAL can target.
2. Decide whether its repo-facing instruction file can reuse `AGENTS.md` or needs another generated adapter.
3. If the runtime supports native commands, generate them from the same shared command templates instead of building a second workflow source. If it does not, install the same baked command skills into the runtime's supported skill surface.
4. Add any config-merge bridge only if the runtime has a stable, user-owned config file that can safely accept additive changes.

### Verify & Self-Check

After changing install or setup logic, verify at least these points:

- the stable repo symlink exists for each supported runtime that needs one
- generated `commands/*/SKILL.md` files no longer contain `{{GAL_ROOT}}`
- Gemini native command files were regenerated from the baked command content
- Antigravity installed skills and agents resolve through `~/.gemini/antigravity-cli/plugins/gal/`
- AGY plugin-root `mcp_config.json` is the sole GAL-managed MCP source for AGY; no GAL-managed MCP entries remain in the global `mcp_config.json` or `settings.json`
- shared skill directories contain reusable skills only, not duplicated command aliases
- MCP reruns update tracked server entries correctly without clobbering unrelated provider-owned config

Before you finish a maintainer change, also ask:

- Did I create a second source of truth?
- Does `/gal` still only solve control-plane problems?
- Do specialist commands still write back to repo-owned files?
- Did I accidentally move state back into a user-global path?
- Can a missing tool still fail loudly instead of pretending to succeed?
- Did I keep collaborative-tool routing at the workflow layer?
- Do README, `commands/commands.md`, and this guide still have distinct jobs?

### Drift Queue

Known navigation/ownership drift to watch when editing install or release docs:

- Provider lifecycle status claims are split across docs and code. Before changing public claims for Claude, Copilot, or Codex, compare [../README.md](../README.md), this guide, and [../crates/gal-engine/src/render/](../crates/gal-engine/src/render/).
- When a translated copy under `docs/i18n/<lang>/` falls behind its canonical source, the translation freshness check flags it; resync before changing public install status claims.

## Conventions

### Documentation Conventions

This section is the authoritative naming and translation policy for `docs/`. The on-location translator signpost is [i18n/guide.md](i18n/guide.md); it points back here and must not duplicate this policy.

#### Durable Documentation Layer (Single Authority)

This subsection is the **GAL-global sole authority** on what constitutes the durable knowledge sink. Other plans and conventions reference this definition; they must not redefine it.

**Durable knowledge sink = `README.md` + `docs/`** (the entire `docs/` tree, at canonical EN paths).

**Excluded from the sink:**

- `.dev/plans/` — transient staging area; plans are deleted after lifecycle closure.
- `.dev/research/` — investigation scratch; promoted facts move to `docs/` or `.dev/plans/`.

**`.dev/project.md` is the compressed index of `docs/`, not a sink.** It summarizes and cross-references durable docs; it does not own facts. Durable knowledge is extracted *into* `docs/`, then indexed *from* `.dev/project.md`. This is consistent with the Knowledge Flow in `conventions/token-budget.md`:

```
Plan ## Status / ## Handoff Notes  →  docs/ (permanent)  →  .dev/project.md (index)
```

Implication: a fact that must survive a plan's lifecycle closure must land in `README.md` or a non-excluded file under `docs/`. Leaving it only in `.dev/plans/`, `.dev/research/`, or `.dev/` is not durable.

**`.dev/project.md`'s re-index has its own bounded content shape.** The re-index is not a free-form summary — its `## Verified Facts` content shape (a fixed one-bullet-per-topic set, current-state-only, durable-pointer form), mutation model (upsert/replace/prune, never append), and hard size budget (30,720 bytes, normalized-LF UTF-8) are owned by `plugins/gal-core/conventions/token-budget.md` § Bounded Current-Topic Index. STEWARD's re-index step must follow that rule, not restate it here.

#### File Naming

- **`README.md` is reserved for the single repo-root README.** No other file in the repo may be named `README.md`. A sub-area entry/index doc uses `guide.md` (a curated signpost) or `index.md` (a generated or listing index) instead.
- **Tool/capability docs** under `docs/collaborative-tools/` use the `-mcp` suffix only when the doc is specifically about an MCP server (`playwright-mcp.md`, `codebase-memory-mcp.md`). A workflow or methodology doc does not take the suffix (`structural-retrieval.md`, `gstack.md`).
- **Skill-aligned docs** match the skill name they support: `docs/collaborative-tools/opencli.md` supports the `opencli-research` skill.
- No two unrelated docs may share the same basename.

#### Multi-Language Translation Policy

The policy is designed so both the set of translated docs and the set of languages can grow without churn.

- **Canonical = English**, at the main filename in its normal location (`README.md`, `docs/manual.md`). Canonical docs are never moved for translation.
- **Translations live under `docs/i18n/<lang>/`** and are named `<name>.<lang>.md`. The folder *and* the filename both carry the language — a deliberate redundancy so a translation file is self-describing out of context, and so the basename is never bare `README.md` (honoring the single-README rule).
- **Mirror-path rule**: root `README.md` → `docs/i18n/<lang>/README.<lang>.md`; `docs/X.md` → `docs/i18n/<lang>/X.<lang>.md` (sub-paths such as `collaborative-tools/` are preserved). The canonical doc's top carries a language-switch link to each translation, and each translation links back.
- **Translatable allowlist** (not "every doc"): currently `README` and `docs/manual.md`, plus a small number of tool docs added on demand. Adding a doc or a language is one entry in `docs/i18n/` — coverage grows visibly and stays bounded. Known near-term language: `ja`.
- **EN-only by default**: maintainer/contract docs — this guide, `collaborative-tools/*` contracts, `research/`, and `plans/` — are not in the allowlist and are not translated.
- `<lang>` tags follow the existing convention in use (e.g. `zh-Hant`, `ja`).

#### Translation Freshness

Every translation under `docs/i18n/` carries YAML front-matter so drift against its English source is visible rather than silent:

```yaml
---
source: README.md          # repo-relative path to the canonical source
lang: zh-Hant
source_commit: <hash>      # the source commit this translation is in sync with; PENDING until stamped
translated_at: 2026-06-02
status: current | stale     # human hint; the check recomputes from source_commit
---
```

`gal translation-freshness` scans `docs/i18n/`, compares `source_commit` against `git log -1 --format=%H -- <source>`, and reports each `(doc, lang)` as `current`, `stale` (hash differs, missing, or a non-hash placeholder like `PENDING`), or `missing` (an allowlisted pair with no translation). It is report-only and scales linearly with languages and docs — it does not extend the structure-map schema.

Operational notes:

- Because GAL commits are authored by a human after review, a freshly synced translation is stamped `PENDING` until the maintainer records the real source commit after committing; the check surfaces those as `stale` so they are not forgotten.
- A translation flagged `status: stale` is knowingly behind its English source. Such files are excluded from the hard repo-wide broken-link gate — the freshness check owns them — until they are re-translated and re-stamped.

#### Planning-Doc OQ Lifecycle

Open Questions in `.dev/plans/*.md` are scaffolding, not a permanent record:

- When an OQ is resolved, bake the ruling into the section that owns it (typically a `## Decisions` table with the ruling, date, and who decided), then **delete the OQ entry** — do not keep `- [x]` OQ corpses.
- When all of a plan's OQs are resolved, do a **full rewrite pass** of the plan instead of incremental patching. Incremental patches leave "see OQ-xx" cross-references pointing at deleted or moved content, which makes plans progressively unreadable.
- A plan body must never require the reader to reconstruct decision history from OQ archaeology; history belongs to git, the plan states only the current ruling.

#### Planning-Doc Item IDs

Requirement, task, and test-point IDs in `.dev/plans/*.md` use **two-digit** numbering (`Rnn`, `T-nn`, `TP-nn`, `OQ-nn`), never three-digit zero-padding (`T-0nn`). Two digits is also the cap: if a plan needs three-digit numbering — roughly 100+ tasks or test points — it is too large for one plan and must be split into multiple plans. A single source plan should stay well within two digits; a task count creeping toward that ceiling is a signal to decompose, not to widen the ID format.

This is a format rule for new and not-yet-implemented plans. Do **not** retroactively renumber an in-flight plan whose IDs are already referenced by commit messages, an execution prompt, or `.dev/executor-logs/` — there the churn outweighs the consistency, and the IDs are load-bearing history.

### Token Discipline

These rules apply to all maintainer and agent work in this repo. The full policy lives in [../plugins/gal-core/conventions/token-budget.md](../plugins/gal-core/conventions/token-budget.md). The developer-facing summary is here.

#### Generated-Artifact Exclusion

Do not read generated adapters (`CLAUDE.md`, `GEMINI.md`, `AGENTS.md`, `.github/copilot-instructions.md`) or build outputs (`bin/`, `obj/`) unless the current task is explicitly about auditing those generated files. They are large, frequently regenerated, and contain no information not already in their source templates.

#### Directed Exploration

Before reading any file, confirm it is named in the current task or is a direct dependency of a task-named file. Stop reading when you have the information needed. Do not load the full codebase as a cold-start step.

#### Failure-Focused Output

When running builds or tests, emit:

- Build: first error with file and line reference. On success, one summary line only.
- Tests: failing test names and assertion messages only. Do not echo passing test names.
- Lint: files and rule violations only. On a clean pass, one summary line only.

Store full logs on disk when needed; retrieve specific lines selectively rather than piping entire logs into context.

#### Context-Pressure Recovery

When context is near the limit during an active task:

1. Write the current task name, last completed step, and any key decisions to `### Handoff Notes` in the active plan's `## Status` section.
2. Write or update the matching plan row in `.dev/state.md` `## Session Continuity` with `Stopped At` and `Next Step`.
3. Do **not** create a separate `CONTEXT.md` file — the plan and state files are the only durable session state stores.

This ensures the next session can resume without re-deriving context.
