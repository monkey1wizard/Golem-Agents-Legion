# gal Architecture

> **A diagram-first map of how GAL fits together, and why.** Structure authority, ownership maps, and architectural decision records. The diagrams carry the explanation. Prose supports them rather than replacing them.
>
> **Audience: anyone.** This document does not assume a source checkout or an intent to modify GAL. A user who wants to know what GAL writes to their machine, which file owns what, or why a boundary exists can read it directly. Modification procedures, rebuild commands, and implementation-level detail live in [`docs/devguide.md`](devguide.md), which is developer-only. Vocabulary is defined in [`docs/naming.md`](naming.md). Contributors start at [`CONTRIBUTING.md`](../CONTRIBUTING.md). Document responsibility boundaries are defined in [README → Documentation](../README.md#documentation).
>
> Canonical language is `en` per `PROJECT_LANGUAGE`. The directed acyclic graph (DAG) source of truth is each crate's `Cargo.toml [dependencies]`. Diagrams are verified against these dependencies. Per-crate roles are sourced from each crate's `//!` documentation.

## Dependency Law & Workspace Crates

`gal-foundation` depends on no other gal crate and serves as the foundation. `dispatch` is also self-contained without internal gal dependencies and acts as a second root. All other crates must depend only downward. Upward dependencies are prohibited. Crates are split to prevent dependency cycles rather than for granularity.

```text
gal-foundation  (foundation — no gal deps)
dispatch        (self-contained — no gal deps)

mcp        ─> gal-foundation
projection ─> gal-foundation
pipeline   ─> dispatch
gal-engine ─> gal-foundation, mcp, projection
cli        ─> gal-foundation, mcp, projection, dispatch, pipeline, gal-engine   (the `gal` binary — aggregates all)
```

| Crate | Single Responsibility |
| --- | --- |
| `gal-foundation` | Foundation: config, ledger, paths, platform, json/env utils, runtime registry, shared MCP **types**, `HealthCheck` trait, render primitives, and `secret_re`. Contains no gal dependencies. |
| `mcp` | **MCP artifact domain**: variable resolution, safe merge, projection, and `HealthCheck` along with per-agent MCP-config serializers (`mcp::serializers`). Projects gal's own MCP servers to each agent. |
| `dispatch` | Headless **executor** invocation: routing, stage-to-role mapping, subprocess spawn with timeout, write-back verification, and per-executor backends. Includes local and SSH remote-execution lanes. A route specifying `sshTarget` and `remoteWorkdir` composes `ssh` as the spawned process (refer to [SSH Spawn Lanes](#ssh-spawn-lanes)). |
| `pipeline` | Local orchestration: task-split and multi-stage dispatch composing `dispatch`, along with task-spec assembly. |
| `projection` | **File projection backend**: converts skills, commands, instructions, and agents into on-disk surfaces via junction, symlink, materialized-copy, or atomic-swap placement for each agent. |
| `gal-engine` | Workflow CLI primitives: canonical-root render, CLI types (`CommandKind`, `ExitCode`, `Action`, `classify_args`), workflow `doctor`, `git_filter` registration, and `translation`. |
| `cli` | The `gal` binary entry point covering subcommand dispatch, `gal doctor` aggregation of all `HealthCheck` instances, and the gal workflow command surface. Hosts internal binary subcommands including the `*-check` family (`finalize-check`, `pipeline-converge-check`, `boundary-check`, `pipeline-preflight`, `planning-check`, `prompt-check`, `refining-check` — refer to [Pipeline Guard Mechanization](#pipeline-guard-mechanization), [gal finalize-check](#gal-finalize-check), and [Planning-Shell Mechanization](#planning-shell-mechanization)), `restore`, and `marketplace-snapshot`. |

> Naming adheres to [`docs/naming.md`](naming.md). Generic bucket names are prohibited. Use `projection` instead of `adapters`, `mcp::serializers` instead of `providers`, and `executor` instead of `dispatch::adapters`.

## Repository & Runtime Topology

### Repository Layout

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
├── crates/          Rust workspace — the runtime (7 crates, DAG + per-crate roles above) · owns render/projection/dispatch/pipeline  [protected*]
├── packaging/       install scripts (install.sh/install.ps1) + publish-public.sh (private→public mirror export) · Homebrew/WinGet manifests are generated by `gal release`
├── docs/            documentation
│   ├── manual.md            user operations manual (canonical EN)
│   ├── architecture.md      this file — structure + ownership + decisions
│   ├── devguide.md          maintainer procedures (inner loop, making changes, operations, doc conventions)
│   ├── integrations.md      public guide for optional integrations (graphify, codebase-memory-mcp, Playwright MCP, OpenCLI)
│   ├── i18n/<lang>/         all translations (mirror layout, <name>.<lang>.md)
│   └── structure/           steward NDJSON structure map + schema
└── .dev/            repo working state
    ├── project.md           compressed project summary + Source Documents index
    ├── state.md             active plans index + session continuity
    ├── plans/<slug>.md       source plans (human-readable, transient) + <slug>.prompt.md execution work files (mutable task memory)
    └── research/            research work files (non-durable — findings promote into docs/)
```

\* Protected paths include `crates/projection/`, `crates/gal-engine/src/render/`, and the `plugins/gal-core/` workflow contracts. Modifying any protected path requires an architect-reviewed plan prior to implementation. Refer to [`docs/devguide.md`](devguide.md) for the change procedures layered over this map.

### `~/.gal/` Runtime Layout

```text
~/.gal/                          machine-local runtime root (never the source of truth)
├── config/                      USER-OWNED machine intent — preserved across upgrade/uninstall
│   └── config.json              machine config: personalization, secrets, profiles, executorRouting (incl. sshTarget/remoteWorkdir)
├── state/
│   └── plugins.lock.json        MIXED-OWNERSHIP: resolver keys (GAL-written) + `_galProjection` (projection registry, GAL-written)
├── plugins/gal/                 GAL-MANAGED canonical plugin root — the runtime content owner
├── generated/                   GAL-PRODUCED projections, rebuildable
│   ├── mcp/managed.json         resolved MCP (SECRET-BEARING, machine-local, never shared)
│   └── runtimes/                runtime projections
└── active/<runtime>/            stable shortcut targets for AI tools (consumers never point at store paths)
```

Provider-visible projections reside outside `~/.gal/` (e.g. `~/.claude/skills/gal`, `~/.copilot/skills/` + `~/.copilot/agents/`, `~/.gemini/antigravity-cli/skills/`) and act as aliases of the canonical root rather than secondary sources of truth. Field-level schemas for the four authoritative runtime files are detailed in [Runtime File Schemas](#runtime-file-schemas) below.

### Schema Precedence Chain

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

## Codebase Ownership Map

This map defines which source file owns which concern. Change procedures are documented in [devguide → Making Changes](devguide.md#making-changes).

### Where Information Belongs

| Information Type | Correct Location |
| --- | --- |
| Durable methodology and contracts | Tracked source docs and source files |
| Repo working context | `.dev/project.md` and `.dev/state.md` in the target repo |
| Human-readable feature plan | `.dev/plans/<plan-slug>.md` |
| Machine-readable execution work file | `.dev/plans/<plan-slug>.prompt.md` |
| Temporary session continuity | `### Handoff Notes` plus `.dev/state.md` |
| Machine-local remote-execution routing | `~/.gal/config/config.json#executorRouting` (`sshTarget`/`remoteWorkdir`) |

Completed plans containing knowledge requiring persistence must be extracted into a durable source file. Leaving the plan as hidden long-term documentation is prohibited.

### Owning Surfaces

| Target Component | Classification Question | Required Source Files |
| --- | --- | --- |
| `/gal` command surface, aliases, or dispatch | Is this control-plane behavior or runtime plumbing? | [../plugins/gal-core/commands/commands.md](../plugins/gal-core/commands/commands.md), [Former Scripts → Owning Crate](#former-scripts--owning-crate) |
| Planning flow or optional-capability semantics | Is this GAL-native planning, an optional capability, or workflow teaching? | [../plugins/gal-core/commands/commands.md](../plugins/gal-core/commands/commands.md), [../plugins/gal-core/conventions/optional-capabilities.md](../plugins/gal-core/conventions/optional-capabilities.md), [../plugins/gal-core/workflows/coding.md](../plugins/gal-core/workflows/coding.md) |
| Install topology, baked command files, or MCP merge | Is this machine-layer install or repo-layer adapter generation? | [Former Scripts → Owning Crate](#former-scripts--owning-crate), `crates/gal-engine` (render), `crates/cli/src/main.rs` |
| Templates and plan lifecycle | Which file should own this information? | [../plugins/gal-core/templates/templates.md](../plugins/gal-core/templates/templates.md), [../plugins/gal-core/workflows/coding.md](../plugins/gal-core/workflows/coding.md) |
| User-facing install documentation | Who owns the words users read? | [../README.md](../README.md) (onboarding entry point), [manual.md](manual.md) (deep operational detail) |
| Source-root resolution (cwd-walk/packaged) | Which doc owns the user's machine intent? | [manual.md](manual.md), [Source-Root Resolution](#source-root-resolution) below |
| Release or channel lineage | Where is the release contract? | `release-<slug>` plan type ([Release Lanes](#release-lanes)), designed by `golem-releaser`. The `gal release` binary subcommand generates artifacts (`CommandKind::Release`), and `.github/workflows/release.yml` is the CI trigger |
| Remote (SSH) execution behavior | Is this part of the main dispatch path or a separate extension? | It is a spawn lane inside `crates/dispatch` ([SSH Spawn Lanes](#ssh-spawn-lanes)), not a separate crate. Refer to `docs/manual.md` → Remote Execution (SSH Dispatch Lane) |
| Non-English planning language authority, hash or parity gates | Is this deterministic (Rust) or semantic (planning-stage command)? | `crates/cli/src/commands/planning_authority.rs` (shared helper), `planning_check.rs`, `prompt_check.rs`, plus [Planning-Language Authority](#planning-language-authority) and [Non-English Planning](#non-english-planning) |

### Former Scripts → Owning Crate

The PowerShell and Bash automation formerly located under `scripts/` is fully ported to the `gal` Rust binary and deleted. Reference this table for the owning crate:

| Former Script Behavior | Current Owner |
| --- | --- |
| `gal <subcommand>` dispatch (`gal.ps1`/`gal.sh`) | `crates/cli` (`gal` binary — chat-control-plane dispatch = `gal dispatch-script`) |
| Shared path and runtime constants (`common.ps1`/`common.sh`) | `crates/gal-foundation` (`platform`, path resolution) |
| `git-filter` registration (manual `git config filter.*`) | `crates/gal-engine` (`git_filter`, `crates/cli` `cmd_filter_transform`) |
| MCP merge (`Update-Mcp.ps1`/`update-mcp.sh`) | `crates/mcp` and `crates/gal-engine` render (`src/render/`) |
| Repo-local adapter generation (`gal init`) | `crates/cli/src/gal/render.rs` — refer to [Repo-Local Adapter Generation](#repo-local-adapter-generation) below |
| Baked-skill generation | `crates/projection` |
| Canonical-root plugin render | `crates/gal-engine` (`src/render/`) |
| `xmachine` remote and local task lanes (`Start-xMachine.*`, `Invoke-Xmachine*`, `Get-Xmachine*`) | Retired. The `crates/xmachine` crate and `gal xmachine` subcommand are removed. Remote execution operates as a spawn lane inside `crates/dispatch` (refer to [SSH Spawn Lanes](#ssh-spawn-lanes)) |
| Task-spec materialization (`New-TaskSpec.ps1`) | `crates/pipeline` (`task_spec`) |
| Install acceptance (`test-install-acceptance.sh`) | `gal doctor` and real-machine acceptance plan |

The only retained non-runtime shell is `packaging/publish-public.sh`, which serves as a curated private-to-public mirror export for operations only and is not a runtime command. Release packaging templates are located under the top-level `packaging/` directory.

### Planning-Language Authority

When `planLanguage != en`, the deterministic layer resides in a single shared helper module `crates/cli/src/commands/planning_authority.rs`. This ensures metadata parsing, rendered-source hashing, and equivalence stamping are never triplicated. Refer to [Non-English Planning](#non-english-planning) below for rationale and boundaries.

```text
planLanguage != en:

  .dev/plans/<slug>.en.md          EN semantic draft — planning-stage semantic authority
        │  render (narrative prose localized, machine anchors stay English)
        ▼
  .dev/plans/<slug>.md             localized source plan + gal:planning-authority metadata block
        │        ▲
        │ hand-  │ reconcile: model merges the edit's intent back into the EN
        │ edit   │ draft, re-renders, then `gal planning-stamp` re-hashes —
        │ detect │ never a silent overwrite
        └────────┘
        │  /plan-to-prompt
        ▼
  .dev/plans/<slug>.prompt.md      English execution prompt
        │
        └─ `gal planning-stamp --equivalence <prompt>` writes prompt-hash +
           equivalence-verdict into the source plan, then the EN draft is deleted
```

- `parse_localized_meta` and `LocalizedMeta` define the localized-source metadata block schema. The six required fields are `semantic-draft`, `planLanguage`, `draft-hash`, `rendered-source-hash`, `prompt-hash`, and `equivalence-verdict`. The block fails to parse if any field is empty. Valid pre-prompt placeholders are `prompt-hash: none` and `equivalence-verdict: pending`.
- `rendered_source_hash` excises the metadata block to prevent self-reference stale states, normalizes LF, applies Unicode NFC, strips trailing spaces, and then computes SHA-256 for cross-machine stability (Windows CRLF equals LF).
- `stamp_equivalence` derives the paired source plan from a prompt path and overwrites its `prompt-hash` and `equivalence-verdict` fields in place. The fields exist only in the source plan. Sibling receipt files are not generated.

Consumers behave as follows: **`planning-check`** validates metadata freshness, EN-draft presence and hash, machine-anchor parity (IDs and file paths exclusively, never prose), and the zh-TW language gate via pure-English-line fraction. A pre-prompt `pending` verdict passes. A **post-prompt terminal state** — `prompt-hash` exactly 64 lowercase hex characters paired with `equivalence-verdict: EQUIVALENT` — also passes `localized-metadata` and `machine-anchor-parity` with the EN draft absent, because `/plan-to-prompt` deletes the draft by design once equivalence is stamped. Any other combination (pre-prompt placeholders, a malformed hash, a mixed hash/verdict pair) keeps a missing draft a hard failure, and the terminal check runs before the self-referential `rendered-source-hash` comparison so an un-restampable hash cannot strand the plan. **`prompt-check`** mandates `equivalence-verdict: EQUIVALENT` alongside a live re-hash match of the prompt body. The write-side operation `gal planning-stamp` computes every hash deterministically. The model never hand-computes hashes. The semantic merge of a localized edit remains within the planning-stage command executed by the model, never in Rust.

### Language Conventions & Personal Layer

The `plugins/gal-core/conventions/` directory serves two independent consumers. These must be considered before adding any new files:

```text
                     consumer 1 — per-repo selector (crates/cli/src/gal/render.rs)
                     `language_tokens` word-boundary match vs `.dev/project.md` `| Language |` row
                     ┌──────────────────────────────────────────────────────────────┐
plugins/gal-core/conventions/*.md ──┬─► selected convention set ──► slim-root adapters (gal init)
~/.gal/local/conventions/<lang>.md ─┤   (source 1 — always-on personal style)
installed agent-plugin skills ──────┘─► Detected Language Skills reference block (source 2 —
                                        name + origin + load-first instruction, content never copied)
~/.gal/local/skills/<name>/ ──────────► loaded by the agent when named (source 3 — on-demand,
                                        never injected always-on)

plugins/gal-core/conventions/*.md ────► consumer 2 — instruction-corpus sweep
                                        (gal-engine render/manifests.rs: copies EVERY .md
                                         unconditionally, no selector logic)
```

Due to consumer 2, example or sample files must not reside inside `plugins/gal-core/conventions/` to prevent unconditional corpus sweeping regardless of language matching. The shipped example `csharp-convention.example.md` is located at `plugins/gal-core/templates/`, which neither consumer reads.

**Dual-source selection and token matching:** The selector tokenizes the `Language` row using `language_tokens`. This process converts characters to lowercase and splits on non-token characters. Token characters include alphanumerics, `#`, `.`, and `+`. Tokens like `c#` or `.net` survive intact. Matching is performed on word boundaries, preventing substring false matches (e.g., `Django` or `MongoDB` will not match `go`). The `rust.md` file selects only on an explicit `rust` token. The no-Language fallback defaults to the neutral trio (`conventions.md`, `token-budget.md`, `working-hours.md`) and excludes `rust.md`. Personal convention files reuse the same tokenizer with a small alias table mapping `c#` or `.net` to `csharp`, `javascript` to `typescript`, and `golang` to `go`. Stems outside the alias table are treated as universal and always match.

**Detected-skill scanner** (`crates/cli/src/gal/skill_discovery.rs`) enumerates four skill roots: the Claude plugin cache (expanded per-plugin), the shared agents skills root (`~/.agents/skills`), Copilot's skills root, and agy's skills root. It parses each `SKILL.md` YAML frontmatter for `name` and `description` using a minimal custom parser without YAML dependencies. Missing or unreadable roots and malformed entries are skipped without panicking. The render layer matches only the skill's name and origin path tokens against the Language line. Descriptions are display-only and never matched. Any name failing a safe charset verification (alphanumerics, `-`, `_`, bounded length) is rejected before reaching a rendered instruction block.

**Personal-layer gates operate on presence.** The personal skills merge, personal MCP merge, and the doctor personal-layer finding key off directory or file existence under `~/.gal/local/` exclusively. There is no `personalLayer.enabled` config key. Placing a file at the personal root activates the feature. An absent path ensures the render remains byte-identical to a Core-only render.

## Projection & Runtime Surfaces

One canonical model maps to multiple agent projections (refer to [Canonical Data Structure](#canonical-data-structure)):

```text
plugins/gal-core/  (source contracts)          ~/.gal/local/  (personal layer, presence-based)
        └───────────────┬──────────────────────────────┘
                        │  canonical-root render (core-wins on collision, Multi-Source Projection)
                        ▼
             ~/.gal/plugins/gal/   canonical root (Claude plugin format, Canonical Data Structure)
                        │  per-runtime projection
        ┌───────────────┼───────────────┬───────────────┬─────────────────────┐
        ▼               ▼               ▼               ▼                     ▼
   Claude Code       Gemini          Copilot          Codex            Antigravity / opencode
   plugin commands   ~/.gemini/      ~/.copilot/      ~/.agents/skills/     ~/.gemini/antigravity-cli/skills/
   + agents (cache:  commands/       skills/ +        (materialized copy)   ~/.config/opencode/commands/
   ~/.claude/        <name>.toml     ~/.copilot/      + ~/.codex/agents/
   plugins/cache/)                   agents/          <name>.toml
```

### Per-Runtime Command Loading

GAL authors control-plane actions as commands (e.g., `/gal status`). Only Claude Code and Gemini possess native slash-command systems. Copilot, Codex, and Antigravity load capabilities as skills. The projection layer in `crates/projection/src/lib.rs` under `update_commands` adapts each command per runtime. The `crates/gal-engine/src/render/` module is not involved because the canonical root retains `commands/` as the source of truth.

| Runtime | Command-Loading Mechanism | Projection Target | Trigger |
| --- | --- | --- | --- |
| Claude Code | Native plugin commands | Canonical-root `commands/` | `/gal status` |
| Gemini | Native TOML slash command | `~/.gemini/commands/<name>.toml` | `/gal-status` |
| Copilot | Skill (no user slash command) | `~/.copilot/skills/<name>/SKILL.md` | `/gal-status` |
| Codex | Skill (custom prompts deprecated) | `~/.agents/skills/<name>/SKILL.md` (official) | `$gal-status` |
| Antigravity | Skill (no native `commands/` folder, commands act as Agent Skills) | `~/.gemini/antigravity-cli/skills/<name>/SKILL.md` | `/gal-status` |
| opencode | Native command | `~/.config/opencode/commands/<name>.md` | `/gal-status` |

**Double-load split:** Runtimes with native command mechanisms such as Gemini, opencode, and Claude do not receive command-skills. The shared `~/.agents/skills` directory is read by codex, opencode, and copilot. The command-skill in that directory is written exclusively when codex is selected. An opencode-only install retains only its native command. The `write_command_skill` function preserves genuine user directories during name collisions using a positive lockfile `is_managed` test to ensure first-run safety. Deselecting a runtime removes its GAL-written command-skill via `remove_gal_command_skill` to prevent lingering files and double-loading.

### Agent Projection Matrix

GAL projects golem agent files and commands or skills per runtime. The projection format adapts the `*.agent.md` source to the target runtime's native agent mechanism instead of copying it verbatim (refer to [Cross-Runtime Role Invocation](#cross-runtime-role-invocation)):

| Runtime | Agent Format | Projection Target | Mechanism |
| --- | --- | --- | --- |
| Claude Code | `*.agent.md` (verbatim with GAL header) | `~/.gal/plugins/gal/agents/` mapping to Claude subagent | Claude native plugin subagents |
| Codex | `*.toml` (flat top-level fields, no wrapper table) | `~/.codex/agents/<name>.toml` | Codex TOML subagent (`crates/projection/src/codex_agent.rs`) |
| Antigravity | None (consumes skills) | `~/.gemini/antigravity-cli/skills/` | Agent surface covered by projected skills, native agent-file load unverified |
| Copilot / opencode | None (no native subagent concept) | N/A | Not projected because skills cover the command surface |

**Codex agent-role schema tracking:** `serialize_codex_toml` writes the six role fields (`name`, `description`, `developer_instructions`, `model`, `model_reasoning_effort`, `sandbox_mode`) at the top level of the file, with no wrapper table around them. Codex validates each role file field by field and stops at the first unrecognized key, so an outdated wrapper key makes Codex reject the whole file at startup and report it through `codex doctor` as a malformed agent role definition. GAL writes exactly one shape, the shape current Codex accepts, and ships no compatibility branch for older readers: a dual-shape serializer would create a permanent maintenance surface for a format GAL does not own, while the single-shape failure mode stays loud and lossless. A future Codex field rename therefore surfaces the same way and takes the same class of fix.

**Dual-mode invocation:** The command `/gal <role>` operates in isolation, returning only verdicts. The command `/gal discuss <role>` operates in-context, loading activation-cores for multi-turn processes.

**Discuss-skill projection for in-context mode:** When Codex is a selected runtime, `gal refresh` writes `discuss-<role>/SKILL.md` to `~/.agents/skills/` for the four planning-review roles (`golem-architect`, `golem-analyst`, `golem-designer`, `golem-releaser`). These skills inject the role's `<role>…</role>` activation-core into the current Codex session without spawning a subagent. The discuss-directory names are added to the shared-skills prune keep-set to prevent eviction during prune operations in `~/.agents/skills/`.

**Orchestrated-only exclusion:** Roles such as `implementer`, `tester`, `auditor`, and `researcher` are excluded from native subagent projection in `crates/projection/src/lib.rs` under `update_agents`. They remain in `KNOWN_GOLEMS` for pipeline dispatch resolution but lack projected `*.agent.md` or `*.toml` files. This prevents bare interactive invocation. A user typing `/gal tester` or `$tester` receives an unknown-intent error rather than executing an unorchestrated tester pass.

**Tool name mapping single source:** Abstract permissions in frontmatter (e.g., `read`, `edit`, `execute`, `search`, `web`) map to valid runtime tool names per runtime via `crates/projection/src/tool_map.rs`. Invocation mode controls sandbox permissiveness. Planning-review roles operate in `read-only` mode for Codex, while builds operate in `workspace-write` mode.

### Repo-Local Adapter Generation

The five repo-local adapter roots (`CLAUDE.md`, `AGENTS.md`, `GEMINI.md`, `.github/copilot-instructions.md`, `.agents/rules/gal.md`) and the two conditional Rust layers (`.claude/rules/gal-rust.md`, `.github/instructions/gal-rust.instructions.md`) generated by `gal init` have a single owner: `crates/cli/src/gal/render.rs`, invoked from `crates/cli/src/init_repo.rs::run_init_repo`. This is a CLI-owned concern. The `gal-engine` and `projection` crates are not involved, as they own machine-level canonical-root and skill, command, or agent projection with a different lifecycle targeting `~/.gal/`. Never route repo-adapter changes through `crates/gal-engine/src/render/` or `crates/projection/`.

```text
.dev/project.md ─► byte-cap check (30,720 B, LF-normalized UTF-8)   fail ─► no output (D24)
      │
      ▼
extract the 8 required H2 sections (exactly once each)              missing/dup ─► refuse,
      │                                                                           name the heading
      ▼
render 5 slim roots  +  2 conditional Rust layers (when Rust selected)
      │
      ▼
validate_root_adapter_budgets (32,768 B per root)  ┐  both preflights complete
preflight_conditional_layers (ownership-marker     ├─ before ANY write — violation
  collision classification)                        ┘  aborts with zero partial output
      │
      ▼
write loop ─► report rows: Written / Unchanged / Removed
```

**Fixed inventory serves as the single source of truth.** The lists `REPO_ADAPTER_ROOTS` (5 paths) and `REPO_ADAPTER_CONDITIONAL_LAYERS` (2 paths) in `render.rs` define the path inventory. This list is shared by rendering, the `gal init` reported output, and the `/gal finalize` idempotency check located at `finalize_check.rs::adapter_paths`. The idempotency check chains both constants instead of maintaining a separate copy. Do not duplicate this list.

**Preflight validation must occur before writing.** The function `render_and_apply_repo_adapters` validates all candidates before executing writes. Root writes precede layer actions within a single call. Cross-file transactional rollbacks are intentionally omitted if a filesystem error occurs after writes commence.

**Report propagation:** The `run_sync` function returns a `ProjectionReport` containing written and removed paths. The module `init_repo.rs` threads this into `InitRepoReport.adapter_report`, and `commands/system.rs` renders it into stdout rows categorized as `Written`, `Unchanged`, or `Removed`. Report shape extensions must occur here rather than by adding secondary output paths.

**Test locations:** Renderer unit and integration tests are housed in `render.rs` under `#[cfg(test)] mod tests`. Lifecycle-consumer tests reside alongside their respective modules in `init_repo.rs` and `commands/system.rs`. The `sync_idempotency` tests in `finalize_check.rs` cover the shared-inventory snapshot.

**Generated-file policy:** All seven adapter paths constitute `gal init` owned output. Hand-editing a repo adapter or conditional layer is prohibited. Hand-edited files bearing the GAL ownership marker are silently overwritten on subsequent `gal init` runs. Files lacking the marker trigger hard collisions. Modifications must target the source files instead, namely `.dev/project.md` for the eight required sections, or `plugins/gal-core/conventions/rust.md` for the conditional layers' body. Re-run `gal init` after modification.

**Situational Rust-convention layering:** Full convention documents are never concatenated into the five slim adapter roots. Two formats deliver Rust-specific context: (1) verified native conditional layers — the two conditional Rust layers named above — for runtimes with primary-source path-conditional loading, each receiving the complete convention body; (2) explicit root-level pointers for Codex, Gemini CLI, Antigravity, and opencode — one-line triggers instructing models to evaluate `plugins/gal-core/conventions/rust.md` before Rust operations, bypassing unverified conditional logic. Runtimes gain situational awareness only when the model processes a trigger line; unsupported native scoping is avoided to keep projection behavior accurate.

### Runtime File Schemas

This section details the structure, required fields, secret boundaries, and precedence rules for the four authoritative `~/.gal/` runtime files.

#### `~/.gal/config/config.json`

Consolidates user-managed machine settings including GAL-core routing fields (`defaultProfile`, `profiles`, `enabledPlugins`, `disabledPlugins`, `executorRouting`), retained machine-local path substitution via `galSkills`, working-hours preferences under `workingHours`, and a `secrets` map for credentials. The deprecated `devMode` and `galRoot` keys are ignored. The `gal doctor` command issues non-blocking warnings for their presence.

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `schemaVersion` | Yes | Integer | Schema version for migrations. |
| `defaultProfile` | No | String | Default profile name falling back to `default`. |
| `profiles` | No | Object | Map of profile names to plugin lists. |
| `enabledPlugins` | No | Array | Explicit plugin selections. |
| `disabledPlugins` | No | Array | Explicit plugin exclusions. |
| `updateChannel` | No | String | Update policy definition. |
| `allowAutoUpdate` | No | Boolean | Per-plugin auto-update override toggle. |
| `executorRouting` | No | Object | Role routing grouped by consumer. Includes `pipeline` for dispatch (`CODER`, `TESTER`, `AUDITOR`), `planning` for review (`ARCHITECT`, `ANALYST`, `DESIGNER`, `RELEASER`), and a shared `executors` default-model block. Serves as the sole routing source. Flat structures are retired, unparsed, and trigger name warnings. The retired standalone routing file is no longer read. |
| `secrets` | No | Object | UPPER_SNAKE keys mapping to secret string values. **SECRET-BEARING** and must not be shared. Values materialize into `mcp/managed.json`. |
| `galSkills` | No | String | Path to the machine-local skills directory representing the only retained machine-path substitution key. |
| `mcpFilesystemPaths` | No | Array | Optional compatibility field output only when filesystem MCP is present. |
| `workingHours` | No | Object | Working-hours enforcement configuration featuring `enabled`, `workdayStart`, `workdayEnd`, `wrapUpTime`, and `hardStopTime`. All time strings use `HH:MM` format. The `enabled` field is a boolean. |
| `memoryHarvest` | No | Object | Contains `{ enabled: boolean }` representing a default-off provider-memory harvest opt-in. This acts as a command-contract field with no Rust code consumption. The `/gal wrap-up` contract reads this file directly. Missing or false values yield no behavior change. |

**Secret boundary:** Values within `secrets` represent the sole secret-bearing fields. Upon materialization into `~/.gal/generated/mcp/managed.json`, the generated file becomes machine-local secret-bearing state. It must never be committed or shared. The file is listed in the GAL shipped `.gitignore`.

**Precedence:** The `config.json` file serves as the user-facing input. Explicit configurations in `enabledPlugins` and `disabledPlugins` override profile-level settings.

**External-note backends** remain outside the Core configuration surface. Their portable contract is defined in `plugins/gal-core/conventions/optional-capabilities.md`. Repo-owned research defaults to `.dev/research/`. Temporary GAL scratch data utilizes repo-relative `.dev/tmp`.

#### `~/.gal/state/plugins.lock.json`

Functions as the projection registry holding the `_galProjection` namespace top-level key. Written by `projection::persist`, this managed-artifact registry records projected paths and source attributions. Each successful persist writes a complete next snapshot: rebuilt categories and their source attributions are reconciled together, while unrelated `_galProjection` categories and non-GAL top-level keys remain intact. It enforces prune-safety by preventing projection rebuilds from deleting non-gal-managed files. It remains deterministic and rebuildable from re-renders.

| `_galProjection` Sub-field | Type | Notes |
| --- | --- | --- |
| `agentProjectionPaths` | Array | Paths for gal-managed agent files. |
| `commandProjectionPaths` | Array | Paths for gal-managed command files. |
| `skillProjectionPaths` | Array | Paths for gal-managed skill files. |
| `discussSkillProjectionPaths` | Array | Paths for gal-managed Codex discuss-skill files. |
| `codexAgentProjectionPaths` | Array | Paths for gal-managed Codex agent files. |
| `legacyProjectionPaths` | Array | Paths for gal-managed legacy or adapter files. |
| `sourceAttribution` | Object | Map assigning paths to source-ids for multi-source projection. |

> gal writes the `_galProjection` namespace exclusively. Any other top-level key in this file is outside gal ownership and is never modified by a gal command.

#### `~/.gal/config/config.json#executorRouting`

Remote execution binds per-role within `executorRouting` rather than utilizing a separate file. A role entry incorporates `sshTarget` and `remoteWorkdir`, which must be provided together. Refer to `docs/manual.md` → Remote Execution (SSH Dispatch Lane) for field contracts and safety boundaries.

**Secret boundary:** The `sshTarget` and `remoteWorkdir` values define machine-local SSH targets and repo paths. Do not share `config.json`.

#### `~/.gal/generated/mcp/managed.json`

This generated file is owned by GAL. It renders from `plugins/gal-core/mcp.json` using machine-local values resolved from `~/.gal/config/config.json`.

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `schemaVersion` | Yes | Integer | Generated schema version. |
| `generatedAt` | Yes | String | ISO 8601 generation timestamp. |
| `generatedBy` | Yes | String | Tool version producing the file. |
| `mcpServers` | Yes | Object | Resolved MCP server configurations. |
| `inputs` | No | Array | Resolved prompt-backed inputs. |
| `_metadata` | Yes | Object | Generation metadata. |

**Server entry structure:**

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `command` | Conditional | String | CLI command absent when `serverUrl` is present. |
| `args` | Conditional | Array | CLI arguments absent when `serverUrl` is present. |
| `serverUrl` | Conditional | String | HTTP MCP endpoint absent when `command` is present. |
| `env` | No | Object | Environment variables strictly resolved without placeholders. |
| `headers` | No | Object | HTTP headers strictly resolved without placeholders. |

**Secret boundary:** Variables such as `context7ApiKey` materialize directly into `headers.CONTEXT7_API_KEY`. Placeholders like `${CONTEXT7_API_KEY}` are eliminated. This file constitutes secret-bearing machine-local state. It must never enter a tracked repo, shared lockfile, or team configuration. Ensure it remains in `.gitignore`.

**Resolved values parameters:** The `mcpFilesystemPaths` field appears exclusively when a filesystem MCP exists in the resolved set. Runtime placeholder resolution is unnecessary because all values materialize fully.

## Key Architectural Decisions

Each decision is identified by its title. Other documents cite a decision by linking to its section heading, so a heading is a stable identifier — renaming one is a breaking change.

Entries are grouped by theme and state the decision, its rationale, and its binding boundary. An entry never restates how a mechanism currently works; the structure sections above own current mechanics. An entry that needs them carries a one-line pointer to the owning section instead of a second copy.

### Canonical Model & Projection

#### Canonical Data Structure

The canonical internal data model for gal utilizes the Claude Code plugin format, comprising `.claude-plugin/plugin.json` and the marketplace schema. Content from gal-core and the optional machine-local personal layer renders once into this canonical form at `~/.gal/plugins/gal`, followed by projection to each agent. A single canonical model acts as input and yields multiple agent projections as output.

- A plugin serves as the distribution container. A skill defined in `SKILL.md` via the cross-tool Agent Skills open standard represents the content. Refer to `docs/naming.md` for definitions.

#### Open-Standards Layer Projection

gal projects `SKILL.md` skills, MCP servers, `AGENTS.md` instructions, and agents. These utilize cross-tool open standards consumed by all supported coding agents including opencode. gal does not generate or execute opencode TypeScript code plugins, which operate as an out-of-scope extension layer.

#### Runtime-Host Consumers

Components constructed specifically to enable full gal usage for a single runtime operate as runtime consumers rather than components of the gal plugin. Examples include VS Code extensions surfacing gal commands inside Copilot Chat. These are situated in surface-namespaced locations and never reside inside the canonical plugin root or a repo-root `extensions/` directory.

- Machine location: `~/.gal/<surface>/…`
- Repo source location: A parallel surface-namespaced directory mirroring the machine layout.

Placing items inside `~/.gal/plugins/gal/…` or a repo-root `extensions/` is prohibited. The canonical plugin root contains only projected plugin assets like skills, commands, agents, and manifests. Hosts reading that projection must exist alongside it rather than within it. The system is currently skill-only with no shipping components, but this decision applies to all future additions.

#### Multi-Source Projection

The skill and agent projection layer employs an ordered source list array `Vec<ProjectionSource>` rather than a single root.

| Type | ID | Persistent | Role |
| --- | --- | --- | --- |
| Core | `"gal-core"` | True | Serves as the fixed baseline. Executes first and undergoes no pruning. |
| Personal | `"local"` | True | Represents the machine-local owner layer under `~/.gal/local/`. Operates based on presence. If the directory or file exists, it undergoes projection. If absent, rendering matches a Core-only render. Config flag gates do not apply. |

**Collision policy applies core-wins, additive-only rules with warnings.** The gal-core serves as the immutable baseline. Non-core sources contribute only unclaimed names. Clashes skip the item and emit a `ProjectionWarning`. Silent overwrites are forbidden.

**Cross-source prune safety:** The `prune_stale_links` function evaluates the complete installed source ID set. Items undergo pruning solely when their `owning_source_id`, recorded in the lockfile `sourceAttribution` map, remains absent across the entire installed set rather than missing from a single invocation.

When an artifact survives deferred prune, the next registry snapshot retains the exact prior record for its managed-path membership and source attribution. This carry-forward applies to every branch that leaves the artifact on disk, including an installed owner and an unauthorized deletion decision; it does not authorize deletion or invent ownership for an unattributed path.

#### Cross-Runtime Role Invocation

Golem agents originate as Claude-format `*.agent.md` source files. Runtime-facing surfaces adapt to each agent's native format rather than copying verbatim. Refer to [Agent Projection Matrix](#agent-projection-matrix) for per-runtime formats, tool name mapping, dual-mode invocation, and orchestrated-only exclusion.

#### Codex Shared-Skill-Surface Boundary

The `~/.agents/skills` directory functions as Codex's official user-skill root and acts as a shared namespace for other skill-managing tooling. Projection into this shared root generates materialized real-directory copies for every core skill using `materialize_skill_dir` with idempotent per-file byte comparisons. Junctions and symlinks are prohibited. The `SkillsProjectionHealthCheck` within `gal doctor` treats the projection lockfile `plugins.lock.json` as the definitive ground truth for missing-skill deletion discriminators. Lockfile-attributed items register as external deletions. Items absent from the lockfile register as not projected or pruned. Missing lockfiles register as unknown causes. A shared-root entry owned by GAL lacking an active command, skill, or discuss-skill counterpart operates as a zombie and undergoes cleanup during `gal refresh` (see § Per-Runtime Command Loading). GAL-private namespaces remain untouched by this decision as they lack third-party co-management and retain junction formatting.

Evidence demonstrated that external tooling selectively deleted junction-based GAL entries in the shared root while real-file entries persisted. Failures align with the projection strategy rather than deleter identities. Real files degrade to stale-and-recoverable states repairable by `gal refresh` rather than undergoing complete deletion.

**Boundary rule:** Lockfile attribution utilizes divergent keys depending on projection write methods. The `write_command_skill` function marks `SKILL.md` file paths, while the caller of `materialize_skill_dir` marks directory paths. Projection-ownership verifications touching this shared root must evaluate both attribution formats.

The `sourceAttribution` map is a subset of the managed-path categories: every attribution key is backed by a managed category path, including the additive `discussSkillProjectionPaths` category. Persisting a complete next snapshot and retaining exact prior records preserve this invariant across shared-root pruning and intermediate writes.

**Lifecycle ownership within the shared root:** the zombie sweep in `update_commands` and the per-source prune pass in `update_skills` both reach the same shared directory through `remove_gal_command_skill`, so exactly one of them must own each entry class. Discuss skills belong to `update_skills`, which already applies the installed-owner and unauthorized-deletion retention rules to them, and the command sweep therefore skips every `discuss-`-prefixed name unconditionally rather than only when Codex is the selected runtime. Making `discussSkillProjectionPaths` a persisted category is what makes this exclusion load-bearing: once a discuss-skill directory carries a category, its `SKILL.md`-keyed record satisfies the command sweep's dual-shape deletion check, and without the exclusion that sweep would delete a directory the skill pass had just decided to keep. The exclusion cannot mask a genuine command zombie, because no name in the command inventory under `plugins/gal-core/commands/` carries a `discuss-` prefix.

### Command Surface & Lifecycle

#### CLI Command Surface Partitioning

The public command surface for `gal` splits into workflow and lifecycle operations.

- **Workflow:** `dispatch`, `pipeline`, `commit-msg`, `dispatch-script`.
- **Lifecycle:** `update` covering version and upgrade instructions, `init` for repo adapter generation, and `doctor` for health checks.
- **Internal subcommands:** These avoid the public `/gal` surface and encompass `clean`, `smudge`, `naming-gate`, `finalize-check`, `pipeline-log`, `release`, `release-notes`, `refresh`, `restore`, `marketplace-snapshot`, `translation-freshness`, `planning-stamp`, `state-merge`, and the `*-check` family.

The `install`, `sync`, and `setup` commands are deleted. Binary installations route through package managers including cargo, winget, Homebrew, and curl. Repo-adapter generation routes through `gal init`. Manual implementations handle `git-filter` registrations.

The projection pipeline triggered by deprecated installation commands obfuscated workflow boundaries. Removal eliminates machine-state mutation code from the `gal` binary and assigns the on-machine lifecycle entirely to the user package manager.

The 26 remaining `CommandKind` variants include `Update`, `Doctor`, `Dispatch`, `Pipeline`, `CommitMsg`, `Clean`, `Smudge`, `Init`, `TranslationFreshness`, `DispatchScript`, `NamingGate`, `FinalizeCheck`, `PipelineLog`, `Release`, `ReleaseNotes`, `Refresh`, `PipelineConvergeCheck`, `BoundaryCheck`, `PipelinePreflight`, `PlanningCheck`, `PromptCheck`, `RefiningCheck`, `PlanningStamp`, `Restore`, `MarketplaceSnapshot`, and `StateMerge`.

#### Source-Root Resolution

The `gal` binary identifies source roots using a `.git`-bounded cwd-walk that falls back to a binary-side packaged layout. Configuration infrastructure spanning `GalMode`, `devMode`, and `galRoot` is deleted, leaving `GalConfig` as an empty, forward-compatible structure. Retired keys present in `config.json` trigger non-blocking warnings via `gal doctor` and undergo silent suppression.

Mode-based source resolution previously mandated user configurations detailing their layout despite environments encoding checkout parameters or packaged binary layouts. Automatic resolution eliminates redundant configuration layers and misconfigurations.

#### Agent Contract Resolution & Delivery

Three components share one boundary, none duplicating another's job. `cli` (`crates/cli/src/commands/dispatch.rs`) owns resolution and reading: it canonicalizes `--workdir`, walks the four-tier precedence from § Source-Root Resolution (`workdir` > `ancestor` > `exe-side` > `embedded`), and reads the winning root's exact `agents/golem-{implementer|tester|auditor}.agent.md` bytes before spec assembly. `pipeline` (`crates/pipeline/src/task_spec.rs`) owns pure assembly only: `TaskSpecInput` accepts the already-read body and renders it verbatim once under `## Agent Contract`, carrying no resolution logic of its own. `dispatch` (`crates/dispatch/src/{cli.rs,run.rs,dispatch.rs}`) owns transport only: it carries one optional coupled `path`+`source` provenance value through local and remote `SpawnConfig` and appends sanitized `contract`/`contract_source` fields to markers and executor-log headers, without learning GAL source-layout resolution itself.

**Precedence trust assumption:** `workdir` outranks `ancestor`/`exe-side`/`embedded` so a trusted local GAL checkout stays authoritative even when a packaged `gal` binary of a different version is also on PATH. A repo vendoring `plugins/gal-core/` always resolves its own contract regardless of the installed binary's version, and `contract_source` is the inspection point for that vendored-checkout version skew.

**Fail-loud corruption rule:** a recognized higher-tier root whose selected phase contract is missing, non-UTF-8, or unreadable is a corruption error that stops dispatch before spawn. It is never treated as absent, and resolution never silently falls through to a lower tier. Total absence across all four tiers is a distinct error listing every tier's outcome plus both recovery routes — reinstall `gal` via the packaging channel, or run from a GAL source checkout.

**Lazy, concurrency-safe embedded tier:** the `embedded` tier is materialized only when every higher tier (`workdir`/`ancestor`/`exe-side`) has already missed — a `workdir`/`ancestor`/`exe-side` hit never touches `~/.gal/embedded-src` at all, and a recognized-but-corrupt higher tier still returns immediately under the fail-loud rule above without materializing. When materialization does run, it is safe under concurrent `gal` processes: each caller extracts into a process-unique staging directory and hands the swap to `gal_foundation::platform::atomic_swap` rather than deleting the canonical target directly. A caller that loses the swap race removes only its own staging tree and rechecks the canonical bytes (with a short bounded retry covering the winner's transient backup-move window), accepting a concurrent winner's byte-identical result as success.

**Inline-size trade-off:** the winning root's contract bytes render verbatim once under `## Agent Contract` in the assembled `TaskSpecInput`, adding roughly 6-14 KB per phase over the advisory 5 KB spec-size target. This is the deliberate cost of making one spec self-contained for both local and SSH-lane dispatch (see § SSH Spawn Lanes) — no dispatched executor is ever instructed to read a control-node-only contract path. No second mechanism runs alongside inline delivery: remote staging, a resolver trait, contract hashing, and a CLI flag were all rejected in favor of embedding the bytes once at resolution time, since remote copy/cleanup carries its own residue failure modes.

#### Local Executor Containment & Bounded Cleanup

Local dispatch contains each executor *before it executes* so a timeout can always reclaim the whole process tree, and bounds every cleanup wait so dispatch can never hang on a child that refuses to die.

- **Pre-execution containment.** On Windows the child is spawned `CREATE_SUSPENDED`, assigned to a kill-on-close Job Object, and only then resumed — any failure along that path (Job creation, assignment, initial-thread lookup, resume) terminates and reaps the suspended child, so containment fails closed rather than leaking a runnable process. On Unix the child is placed in its own process group (`process_group(0)`), and timeout termination targets the negative PGID to reach the whole group, not just the direct child. Deliberate session/process-group breakaway by the executor itself is outside this guarantee.
- **Bounded timeout cleanup.** On a timeout, primary termination (Job Object terminate / negative-PGID kill) fires, then dispatch waits only through fixed grace periods on the existing wait channel — never an unbounded join. Drain threads are joined only after exit is confirmed. If cleanup stays unconfirmed even after the Windows-only `taskkill /F /T` fallback, dispatch returns `DisconnectedPartial` with a `cleanup-unconfirmed` reason and leaves the drain/writer threads unjoined rather than block. The Windows Job handle is closed exactly once on every path via a `Drop` guard.
- **Non-blocking spec delivery.** stdout/stderr drains start before the spec is written; the stdin write runs on a dedicated thread reporting `complete`/`failed`/`pending` over a one-shot channel, so a multi-megabyte spec to a child that never reads stdin cannot stall the timeout clock. Timeout cleanup never waits on the writer.
- **Observation-only timeout evidence.** Timeout reasons state only what was observed — output-seen-or-not, `stdin_delivery=<status>`, `spec_bytes=<n>` — and never infer a cause (an interactive prompt, a hang). The `timeout-no-output` / `timeout-midrun` state tokens are retained for log compatibility but carry no causal claim.
- **Per-role timeout override.** `executorRouting.pipeline.<ROLE>.timeoutSecs` (optional positive integer) overrides the dispatch timeout for that role on both local and SSH routes; absent, the unchanged 300-second default applies; `0` is rejected with a warning at load. Operator field contract: `docs/manual.md` → Executor Routing.

#### Rebuild Primitives via `gal refresh`

The `gal refresh` command serves as an internal binary subcommand that re-orchestrates existing public functions to rebuild local environments. It is not part of the public `/gal` surface. It rebuilds the canonical plugin root at `~/.gal/plugins/gal`, the parent Claude marketplace manifest at `~/.gal/plugins/.claude-plugin/marketplace.json`, and per-runtime projections across all five runtimes.

**Boundary:** The `gal refresh` operation writes solely to the GAL-owned `~/.gal` tree. It bypasses `~/.claude/plugins/cache` and `installed_plugins.json`, which remain Claude-managed. Claude integrates updated plugins upon restart or re-addition.

The removal of legacy install and sync commands eliminated CLI entry points for rebuilding derived content after updates or deletions. The `gal refresh` command fulfills this requirement as a minimal rebuild primitive without assuming new authority. Selective per-runtime projections remain deferred enhancements.

#### Marketplace Snapshot

The `gal marketplace-snapshot --source <dir> --out <dir>` command operates as a publisher-internal binary subcommand. It renders the public GAL plugin tree to explicit output directories and generates Claude marketplace catalogs. Invoking `render_canonical_root_to(source, out/gal, include_machine_local=false)` ensures snapshots contain only core GAL content. Personal layers, machine-local MCPs, host binaries, runtime projections, and `~/.gal` writes are excluded.

Separating this function from `gal refresh` prevents unrelated projection side effects during CI publishing operations. Explicit output paths maintain deterministic, platform-neutral publishing capabilities.

The marketplace plugin functions as a discovery and instruction carrier rather than a comprehensive GAL installation tool. The `install-gal` skill remains model-invocable, instructing AIs to display exact package-manager commands, secure user consent, execute available installations, and verify operations via `gal --version`. Dedicated install commands and GAL-side installation state machines are omitted.

### Pipeline & Check Mechanization

#### Pipeline Guard Mechanization

Pipeline enforcement points encompass Step-1 prerequisites, pre-commit allowlists, and convergence verifications. These are mechanized as pipeline-internal binary subcommands: `gal pipeline-preflight`, `gal boundary-check`, and `gal pipeline-converge-check`. Each generates a machine receipt serving as the sole pass-basis for the corresponding SKILL gate. Self-reporting is prohibited. A `not-run` status invariably exits non-zero and constitutes a failure.

**Three-Zone Principle:** Read-only verification maps to mechanized binaries using receipt-based evaluations. Semantic judgment maps to AI responsibilities covering orchestrator correctness gates and goal-backward verifications. Destructive actions map exclusively to orchestrators handling commit boundaries. The three `*-check` binaries operate strictly within the verification zone without executing commits or judging semantics.

**Scope boundary:** Missing or empty `## Affected Files` sections force `boundary-check` to exit as `not-run`. This invariant prevents omitted allowlists from bypassing boundary gates silently.

Mechanized binaries mirror the receipt-gate pattern utilized in `/gal finalize`, enforcing zero-trust verification symmetrically. Check primitives encompassing `Receipt`, `CheckState`, `CheckOutcome`, `check_three_surface`, `checked_task_ids`, and `commit_note_hash` are shared via `pub(crate)` functions from `crates/cli/src/commands/finalize_check.rs`.

#### `gal finalize-check`

The `gal finalize-check` command integrates eight checks, combining five foundational assessments with three read-only informational evaluations.

- **check(e) `cited-test-existence`:** Harvests backticked Rust identifiers from the plan's `## Test Results` window and resolves each as an `fn`, a `mod`, or an `<name>.rs` file under `crates/`. Harvesting is **task-scoped**: a plan declaring only `manual` probes keeps its whole-plan `skipped:` early return, but a mixed plan is partitioned into the window's preamble plus one chunk per `### ` heading of any form, and a chunk is excluded only when every `## Test Plan` row whose `Covers` cell explicitly names that task declares `manual`. Preamble prose, chunks whose heading names no task, and tasks with no explicit covering row all stay harvested, so the residue is fail-closed. Within a harvested chunk the scan is per-line with fenced blocks skipped, and a token whose 40-character prefix window carries an existence-negation marker is checked in the **inverted direction** — resolving nowhere is a pass — so evidence asserting a symbol no longer exists is not read as a hallucinated citation. A positive citation of the same token anywhere else overrides the negation. Both fail-open lanes are disclosed rather than silent: every summary reports `resolved`, `missing`, `skipped-manual-task`, and `negative-cited`. Rationale: prose evidence legitimately cites non-function tokens (front-matter keys, config names) and legitimately asserts absence, and treating either as a hallucinated test name blocked landings that were in fact honest.
- **check(f) `finalize-mode`:** Executes `git branch --show-current` to output `worktree` or `already-on-main`. The state remains `Pass` and functions purely as information rather than a gate condition. The `/gal finalize` command reads the `summary` field to assess applicability for merges and teardowns.
- **check(g) `executor-log-scan`:** Scans per-checked-task rather than performing a whole-root analysis. Every `[x]` task requires bound convergence characterized by three-surface agreement, task-commit existence, and cleared cursors before dispatch evidence is trusted. Required phases comprising `implement`, `test`, or `audit` classify into specific outcomes derived from the Execution Outcome Taxonomy, such as `in-conversation`, `dispatch-offload`, or `recovered-in-conversation`. Unterminated `started` markers constitute failures regardless of convergence status. Missing, malformed, unbound, or foreign evidence fails the check comprehensively, identifies the task, and registers zero checked tasks as `NotRun`. **Accepted trust-boundary limitation:** Scans exclusively evaluate the plan-scoped attempt-log directory assigned to the target. Legacy top-level logs outside plan-scoped subdirectories remain undetected and classify as `in-conversation` silently, as they lack metadata for safe scoping.
- **check(h) `durable-layer-commit`:** Reads the hash supplied via `--durable-commit` and performs a `git cat-file -e` existence check against it. The check is registered only in `--hygiene-only` mode. An omitted flag returns `NotRun` (`not supplied`); a supplied hash returns `Pass` (`exists: <hash>`) or `Fail` (`missing: <hash>`). Receipts certify existence exclusively.

**Evidence production:** Dispatch processes allocate unique, collision-safe attempt paths and write `terminal_state: started` markers before spawning processes or initiating availability checks. Write failures block spawning. Failed terminal rewrites preserve `started` markers as evidence of unterminated attempts. Log directories remain plan-scoped when pipeline inputs are Prompt or SourcePlan paths. During finalize operations, check(g) bypasses pipeline receipt files to enforce zero-trust doctrines, re-establishing bound convergence in-process and rescanning plan-scoped directories.

**Verify and destructive boundaries:** check(h) asserts existence without authorizing or triggering plan-file deletions. Deletions remain orchestrator-executed operations requiring independent hash confirmations and durable-layer commit hash signals from STEWARD, passed to check(h) via `--durable-commit`.

The addition of append-only checks ensures the byte-level stability of foundational checks referenced by all active finalize procedures. Informational checks operate safely in any checkout state without independent blocking logic.

#### Finalize-Side `.dev/state.md` Merge Resolver

`gal state-merge` is a finalize-internal, zero-argument binary subcommand that deterministically resolves a merge conflict when the entire unmerged path set is exactly `{.dev/state.md}`. It runs from `/gal finalize` Sequence 3, before the merge commit, only for that one exact case; every other conflict shape keeps the existing unconditional STOP.

**Why a resolver, not a merge driver.** GAL runs multiple plans concurrently in separate worktrees, and every task's converge commit rewrites `.dev/state.md`'s plan-keyed tables. Merging a plan branch back to main therefore conflicts on `.dev/state.md` often, even though the conflicting rows are usually disjoint — a plain three-way line merge fails on adjacent table rows even when the actual edits don't overlap semantically. Registering a git `merge.*.driver` was considered and rejected: it would add `.gitattributes` config, PATH/version-skew exposure across machines, and a `gal doctor` check surface, for a mechanism external to the binary that executes finalize. Since the finalize binary and the resolver are the same binary by construction, invoking the resolver as an internal subcommand from Sequence 3 needs none of that — no config, no attribute, no doctor row, no PATH resolution.

**Two structural alternatives were also rejected**, recorded for future reference:

- **Per-plan state files** (`.dev/state/<slug>.md`) would make concurrent plans touch disjoint files, but `.dev/state.md` is a single bounded-size contract every consumer reads as one file — cold-start order, `/gal whats-next` and `/gal wrap-up`'s first-non-terminal-row selection, the `state-bound` hygiene check, and the template all assume one file. Splitting would move the merge problem into a cross-file index (`## Recent Close-outs` ordering and `## Active Plans` priority are inherently global) for a contract-surface change far larger than a resolver.
- **Regenerating `.dev/state.md` as a derived index** (the `.dev/project.md` pattern — derived files are not merged, they're regenerated) was rejected because `.dev/state.md` is not fully derivable: `## Recent Close-outs` rows outlive the plan files they describe (deleted at finalize time), `## Global Decisions` and Session Continuity `Context` cells are hand-authored prose, and Active Plans row order encodes an owner-set priority a regenerator has no second durable source for.

**Merge semantics.** The resolver reads base/ours/theirs only from git index stages `:1:`/`:2:`/`:3:` (never parses conflict markers), normalizes CRLF to LF, and splits the file into a fixed recognized-section shape. Plan-keyed sections (`## Active Plans`, `## Recent Close-outs`, `## Parked Plans`, `## Session Continuity`) merge row-by-row by their contract-defined key column, with a surviving-base-key reorder guard on Active Plans and Session Continuity (owner-set priority order must not be silently discarded). Every other region — the preamble, whole non-keyed sections, and a keyed section's own scaffolding (header/separator/trailing comments) — merges with plain three-way semantics. Any ambiguity (divergent edits, unknown/duplicate section shape, cell-count mismatch, duplicate keys, an invalid `Recent Close-outs` date, or the merged file exceeding the 16,384-byte normalized-LF budget) fails closed before any write.

**Write honesty.** The commit path snapshots worktree bytes and unmerged index-stage entries before any mutation, writes and reads back a same-directory temp candidate, backup-moves the conflicted file, installs and reads back the candidate, runs `git add`, and verifies the exact success postcondition (one stage-0 entry whose blob matches the candidate, no unmerged entries, worktree bytes match). Any failure past the backup-move restores worktree bytes and reconstructs the original index stage entries via `git update-index --index-info`, then only reports `STATE_MERGE: unresolved` (repository proven byte-identical to before the call) once that restore itself is verified exact — an unproven restore reports `STATE_MERGE: rollback-unconfirmed` instead, so finalize never mistakes an unconfirmed rollback for inertness. This mirrors the check-mechanization Three-Zone Principle above: read-only verification (parsing, merging, postcondition checks) is mechanized; the orchestrator-owned commit boundary is preserved (the resolver never runs outside the finalize merge step, and never issues `git commit`).

#### Planning-Shell Mechanization

Planning-stage deterministic shells are enforced by three internal subcommands: `gal planning-check`, `gal prompt-check`, and `gal refining-check`. These combine with `finalize-check`, `pipeline-converge-check`, `boundary-check`, and `pipeline-preflight` to establish a uniform receipt-driven checking ecosystem. Deterministic structures reside in the binary, while judgment persists with the AI.

**Scope split:**

- `planning-check` regulates planning and deep-planning handoffs by evaluating `OQ=None`, requisite sections, `ARCH_REVIEW: CLEAR` statuses, naming-gates, and `planLanguage` consistency. Two rows govern the `## Approval` block. `approval-shape` requires exactly four lines in the fixed order `Human approval`, `Architect review`, `Design review`, `Business review`, each matching `- <label>: [<token>]` with an optional ` — <reason>` tail, and each token drawn from that field's closed set (`[pending|approved]`, `[pending|clear|blocked|not-required]`, `[not-requested|clear|blocked]`, `[not-requested|clear|blocked]`); the section scan stops at a `---` thematic break so an execution prompt's divider is not read as a fifth field. `arch-review-consistency` is a separate biconditional row: `Architect review: [clear]` holds if and only if a standalone `<!-- ARCH_REVIEW: CLEAR -->` marker exists, catching token and marker drifting apart in either direction, which `arch-review-clear` alone cannot see because it only inspects the marker.
- `prompt-check` regulates compressed execution prompts by ensuring machine-anchor preservation without authorizing transformations or rewrites.
- `refining-check` regulates refined source-plan structures by mandating task counts between 1 and 99. Zero tasks trigger failures. It evaluates pairing logic, ID well-formedness, path references, and `ENG_REVIEW: CLEAR` statuses.

Planning flows previously utilized prose-only compliance. These binaries mechanize the deterministic shell and explicitly prevent Rust from assuming ownership of semantic compression, task design, or OQ judgments.

#### Repo Hygiene

Four `gal finalize-check` evaluations reside in `crates/cli/src/commands/finalize_hygiene.rs` as a `cli`-crate module rather than a distinct crate. Two evaluations remain universal, confirming `.dev/project.md` and `.dev/state.md` presence in all initialized repositories. Two evaluations are scoped to GAL-source checkouts, registering as absent rather than `NotRun` in all other locations, preventing GAL contract hygiene from blocking downstream repositories.

**Finalize time-of-check ordering:** The full-mode `gal finalize-check` executes prior to holistic reviews, doc-syncs, and merges. A secondary `--hygiene-only` receipt executes post-write operations for `.dev/state.md` and pre-deletion operations for plan files to catch drift from fix commits or merges. Divergent default receipt paths prevent mutual overwrites. **The hygiene-only mode operates with diminished scope,** running the applicable hygiene rows plus the `durable-layer-commit` row (check(h)) and strictly avoiding equivalence claims with the full mode. The `mode: full|hygiene-only` line within `render_finalize_receipt` functions as load-bearing evidence during evaluations.

**Memory parameters for `.dev/state.md`:** Fixes for unbounded per-plan history expansion implement content-shape bounds enforced within the file. Parameters require a `## Recent Close-outs` table with a maximum of two rows, a 16,384-byte limit normalized for LF, and zero exact-predicate legacy comments. Data storage via databases or providers is prohibited. This corresponds with [Bounded Current-Topic Indexes](#bounded-current-topic-indexes) methodologies utilizing check-time byte caps and unified authority models.

Markdown-link scanners and roster-parity parsers function as hand-written line and byte scanners without additional dependencies. Maintainer-facing implementations inhabit [devguide → Making Changes](devguide.md#making-changes).

### Planning Language

#### Non-English Planning

When `planLanguage != en`, planning delegates to three distinct authorities: the EN semantic draft, the localized source plan, and (post-prompt) the English execution prompt. For `en`-prefix configurations, the source plan operates as its own EN authority, discarding the draft layer. This layering exists because narrative prose in a non-English plan is unreliable as an exact technical-semantic record across model sessions — the EN draft anchors semantic intent while the localized plan stays the human-facing surface. See § Planning-Language Authority for the file flow, metadata schema, and hashing mechanism.

**Deterministic and semantic boundaries:** The Rust check layer handles deterministic tasks covering metadata parsing, normalized hashing, hash freshness verifications, machine-anchored parity validations focusing strictly on IDs and paths, language heuristics, inline equivalence verdicts, and live re-hashes of prompt bodies. Semantic merges of localized edits remain within the AI model's purview.

Evidence proved pure non-English planning degrades technical semantics across sessions. Establishing an EN semantic authority resolves this deficit. Language gates utilize pure-English-narrative-line fractions to prevent false positives generated by letter-ratio gates evaluating dense inline technical terminology. Machine anchors remain strictly English to ensure compatibility with `prompt-check` and `finalize-check`.

### Conventions & Repo Adapters

#### Language Convention Supply Chain

GAL Core excludes personal coding-style conventions and supplies only `rust.md` representing the internal GAL dev standard. Language house styles populate downstream repositories via personal convention files under `~/.gal/local/conventions/<lang>.md`, installed agent-plugin detections supplying read-only reference blocks, and on-demand personal skills located at `~/.gal/local/skills/`.

**Flag removal:** Personal layers previously mandated both a directory and a `personalLayer.enabled` configuration flag. The system now utilizes presence-based evaluations. Files positioned at the personal root constitute an opt-in action, mirroring machine-local GAL surfaces.

The personal-conventions integration operates in the non-Protected CLI render layer within `crates/cli/src/gal/render.rs` and requires zero new crates or config keys. A `| Personal Conventions | off |` specification in `.dev/project.md` disables personal styles for specific repositories, ensuring public repositories omit owner-machine context. Installed-plugin detection remains read-only. Safe charset evaluations filter names before they reach instruction surfaces to neutralize prompt-injection vectors. Staleness conditions require `gal init` re-executions rather than background watchers.

#### Bounded Current-Topic Indexes

The `## Verified Facts` section in `.dev/project.md` functions as a fixed current-topic index restricted to ten topics. Each topic utilizes a single bullet detailing current states coupled with a durable `README.md` or `docs/` pointer. Topics require upsert, replace, or prune operations. Append operations are prohibited. Size budgets and models rely on `plugins/gal-core/conventions/token-budget.md` § Bounded Current-Topic Index.

**Render-choke enforcement:** `.dev/project.md` must stay under the byte cap enforced by `crates/cli/src/gal/render.rs` preceding adapter renders (see § Repo-Local Adapter Generation for the exact cap and flow). Violations abort rendering completely, preventing partial outputs. This bounds `.dev/project.md` growth at the source rather than only at render time.

**Post-doc-sync sequencing:** During `/gal finalize`, STEWARD reindexes `.dev/project.md` before capturing adapter-render idempotency evidence.

Imposing a strict render-time choke point prevents `.dev/project.md` from expanding into an unbounded history sink that exceeds downstream context limits.

#### Repo-Local Adapters

Repo-local adapters render as bounded slim cores rather than concatenating full convention documents, and situational Rust-convention context splits across verified native conditional layers and explicit root-level pointers. See § Repo-Local Adapter Generation for the byte budget, rendering mechanics, and the two-format layering split.

**Lifecycle boundary:** the CLI dependency boundary ensures repo adapters bypass machine projection components entirely. Processes execute through `crates/cli/src/gal/render.rs` without dependencies on `crates/gal-engine/src/render/` or `crates/projection/`.

**No-immunity boundary:** the byte cap targets GAL-generated content operating under documented default context budgets. It does not provide truncation immunity against user-lowered limits or external runtime context configurations. The two conditional layers are exempt from this cap.

### Release & Deploy

#### Release Lanes

Releases function as first-class `release-<slug>` plan types independent of `/gal pipeline` or `/gal finalize` phases. The `golem-releaser` operates as a planning-stage review role limited to advisory capacities with abstract tools mapping to `read`, `search`, and `web`. Execution permissions are omitted.

**Lifecycle:**

1. The `/gal releaser` assesses release APIs and CI tooling, designs workflows, and issues advice without writing files or executing actions.
2. The `/planning release-<slug>` operation materializes advice into source plans.
3. Standard `/gal pipeline` workflows orchestrate execution via `golem-implementer`, `golem-tester`, and `golem-auditor`.
4. Finalization processes via `/gal finalize` conclude release plans.

Release tooling and artifacts integrate directly into plan execution. The finalize stage remains a universal landing step unburdened by disparate release targets. The `Phase` enumeration strictly bounds to `Implement`, `Test`, and `Audit`.

#### Deploy Capability Verification

The `golem-releaser` explicitly confirms deploy targets with users before executing research or design protocols. After confirmation, capabilities are resolved via MCP manifests, skills directories, API web-research, and CLI configurations. User assertions represent the sole authority for target verification.

Deploy target architectures encompass GAL-self CLI binary releases and downstream repository endpoints. Operational models terminate at design advice, precluding `golem-releaser` from engaging in canary deployments, rollbacks, or production monitoring.

#### Public Release Chains

GAL public release chains initiate from singular human actions pushing approved `v*` tags to private GitLab instances. Processes sequence through GitLab CI evaluations, curated snapshot publishing, and GitHub Actions deployments covering Homebrew taps, winget submissions, and smoke tests. Prerelease rehearsals validate chains without designating stability. Artifact layouts maintain unversioned `gal` binaries paired with packaged GAL payloads.

Release binaries compile with static CRT contracts on Windows. This operational requirement prevents sandbox failures caused by missing VC++ Redistributable dependencies.

#### Verified Release Body Gate

`gal release-notes [<from>..<to>]` drafts a deterministic CHANGELOG section from a commit range (default: the highest HEAD-reachable stable `vMAJOR.MINOR.PATCH` tag through `HEAD`). It drops process-noise commits against a frozen allowlist, groups survivors into `Added`/`Changed`/`Fixed`/`Other` by Conventional Commit type and breaking-change markers, and prefixes every surviving item with a `[[GAL-RELEASE-DRAFT:<short-hash>]]` sentinel plus a `[REWRITE]` (naming-gate violation) or `[REVIEW]` (unrecognized type) flag where applicable. The draft is never final text — a maintainer curates it into `CHANGELOG.md`'s dated `## [X.Y.Z]` section, clearing every sentinel and flag before the release tag.

`gal release` gates on that curated section for any stable (non-prerelease) version: it reads `CHANGELOG.md`, extracts the exact `## [<bare-version>]` heading's body, and fails before writing any artifact if the section is missing, empty, or still carries a `[[GAL-RELEASE-DRAFT:` sentinel. On success it writes the section verbatim as `release-out/release-body.md`. `.github/workflows/release.yml`'s `sign` job passes that path as `body_path` to `softprops/action-gh-release` for stable tags (`generate_release_notes: false`), while prerelease tags keep GitHub's generated notes and never reference the stable body — the same `-`-in-version predicate governs both the local gate and the workflow's `!contains(github.ref_name, '-')` conditionals, so they cannot disagree on which tags are stable.

### Remote Execution

#### SSH Spawn Lanes

Cross-machine task execution operates as a supplementary spawn lane within `dispatch::run`. When `config.json#executorRouting` role entries define `sshTarget` and `remoteWorkdir` pairs, executions resolve as `ssh <target> "cd <remote-workdir> && <executor> <args>"` running on synchronous held sessions. Prior architectures utilizing `zellij` or `xmachine` polling logic are retired.

The task spec fed to the remote executor is the same self-contained `TaskSpecInput` produced on the control node — its `## Agent Contract` section already carries the winning root's contract bytes inline (see § Agent Contract Resolution & Delivery). The remote machine never opens a control-node-only contract path over SSH; it reads only what already arrived in the spec.

```text
control node                                remote machine (GAL-dedicated checkout)
────────────                                ──────────────────────────────────────
resolve route (sshTarget + remoteWorkdir)
      │  guard: remote HEAD == control HEAD
      │         && working tree clean          fail ─► REMOTE_GUARD_FAILED (loud)
      ▼
ssh <target> "cd <workdir> && <executor> <args>"
      │        (held synchronous session)      executor runs, writes receipt
      ▼
fetch receipt + uncommitted diff  ◄────────────┘
      │  apply diff on control node
      ├─ apply OK  ─► ssh: git reset --hard && git clean -fd   (remote back to clean)
      └─ apply FAIL ─► remote checkout left intact for inspection (never cleaned)
```

Integrating remote operations directly into the live dispatch path minimizes architectural deviations. The remote lane relies strictly on synchronous spawns. Disconnect-survival features are excluded, restricting remote lanes to single-dispatch workflows requiring manual resynchronization. Support remains limited to POSIX remote systems.

### Runtime Reliability & Review Method

#### Adversarial Review Methodology

Adversarial review logic is centralized within `plugins/gal-core/skills/adversarial-review/SKILL.md` and accessed via pointer references from review roles. The skill defines steel-man logic, refutations under doubt, explicit verdicts encompassing `APPROVE`, `REVISE`, or `REJECT`, and jidoka stop-line constraints. `NotRun` states constitute automatic failures.

The methodology centralizes execution frameworks while preserving domain-specific judgments. Role lenses dictate architecture, business, UX, documentation, or testing evaluation parameters locally. The `projection::codex_agent::generate_discuss_skill_md` logic injects role sections into discuss skills directly, necessitating embedded pointers.

#### Named Workflow Obedience

Workflow obedience is enforced at two levels. The `AGENTS.md` preamble explicitly renders Named Workflow Obedience rules via `crates/cli/src/gal/render.rs` within the first segment of the file. Command-skill frontmatter descriptions prioritize execution triggers and activation terms.

Prose embedded within workflow skill bodies is excluded from enforcement mechanisms. Machine gates mandate `gal pipeline-preflight` receipt generation before any implementation edits.

#### Pre-Rendered Narration

A third enforcement level applies where the required output is user-visible text rather than a file write. A skill rule that only *describes* what the orchestrator should say is unenforceable prose, and the observed failure mode is paraphrase: dispatch announcements drifted into claiming background execution, later notification, or continued work during the wait — none of which the pipeline's sequencing actually has. The countermeasure is to move the text itself into the binary. `gal dispatch-script` pre-renders a complete `REPORT_LINE` field in the `OFFLOAD` block with the live phase, task, role, executor, model, and effort already substituted, and the skill contract reduces the orchestrator's obligation to copying that one line verbatim, exactly once. Deciding what to say becomes a rendering concern with test coverage, and only the act of copying stays in the model's hands.

The same rendering rule governs the routed `Dispatch:` marker suffix. Its sanitized ` effort=<value|(default)>` field is computed once immediately after route resolution and reused by every later success or degrade marker in that run, so no branch can report an effort the run did not attempt. Markers emitted before a route resolves — raw or direct dispatch, and the `no-routing` degrade — keep the provenance-only suffix and stay byte-identical to the pre-effort format. Every value crossing into either surface from `config.json` is flattened to one logical line first (`single_line` on block fields, `sanitize_header_value` on marker suffixes), so an embedded newline can never forge a second field or a second marker line.

#### Codex Pipeline Reliability

Codex-specific execution failures translate into durable machine-checked diagnostics operating across three distinct layers.

1. **Prompt/tool surface:** Named Workflow Obedience bullet points and GAL critical runtime packs render within the `AGENTS.md` preamble.
2. **Projection/runtime surface:** The `SkillsProjectionHealthCheck` assesses projected skill states relative to canonical sources.
3. **Pipeline-execution surface:** SKILL templates encapsulate PowerShell-safe `#file:<prompt>` quoting, explicit prompt selection criteria, and sandbox write-denial stops.

Diagnostics prevent silent degradation caused by unquoted variables, restricted sandbox access, multi-plan ambiguities, or disabled memories.
