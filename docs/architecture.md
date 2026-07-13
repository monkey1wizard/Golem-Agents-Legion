# gal Architecture

> **Structure authority** for the gal Rust workspace: the crate dependency-law DAG and the key
> architectural decisions. For **vocabulary** (what every term means), see [`docs/naming.md`](naming.md);
> this doc does not redefine terms.
>
> - Canonical language: `en` (per `PROJECT_LANGUAGE`).
> - **This doc describes the current architecture** after the gal/ccync split
>   (`refactor-gal-deletedown-drop-ccync-tree`): gal sheds all third-party-plugin
>   *management* to the ccync product and keeps only the workflow + the cross-agent
>   projection engine. `base` was forked to `gal-foundation`; the `ccync-foundation`
>   and `setup` crates and the `gal-engine` management modules (catalog/adopt/
>   reconcile/release) were deleted.
> - **DAG source of truth = each crate's `Cargo.toml [dependencies]`**; the diagram below is verified
>   against them, not hand-drawn. Per-crate roles below are sourced from each crate's `//!` doc.

## Dependency law

`gal-foundation` depends on no other gal crate — it is the foundation. `dispatch` is likewise
self-contained (no gal-internal deps), a second root. Every other crate may depend only **downward**;
never upward. Crates are split to break dependency cycles, not for granularity.
**DAG source of truth = each crate's `Cargo.toml [dependencies]`** — the edges below are verified
against them:

```text
gal-foundation  (foundation — no gal deps)
dispatch        (self-contained — no gal deps)

mcp        ─> gal-foundation
projection ─> gal-foundation
pipeline   ─> dispatch
gal-engine ─> gal-foundation, mcp, projection
cli        ─> gal-foundation, mcp, projection, dispatch, pipeline, gal-engine   (the `gal` binary; aggregates all)
```

## Workspace crates (7)

| Crate | Single responsibility |
| --- | --- |
| `gal-foundation` | Foundation: config, ledger, paths, platform, json/env utils, runtime registry, shared MCP **types**, `HealthCheck` trait, render primitives, `secret_re`. No gal deps. |
| `mcp` | **MCP artifact domain**: variable resolution, safe merge, projection, `HealthCheck` — plus the per-agent MCP-config serializers (`mcp::serializers`). Projects gal's *own* MCP servers to each agent. |
| `dispatch` | Headless **executor** invocation: routing, stage→role, subprocess spawn + timeout, write-back verification, per-executor backends — local **and** the SSH remote-execution lane (a route with `sshTarget`+`remoteWorkdir` composes `ssh` as the spawned process; see D22, `docs/remote-execution.md`). |
| `pipeline` | Local orchestration: task-split + multi-stage dispatch composing `dispatch`; task-spec assembly. (The prior `Transport` trait scaffold — see D22 — was retired with zero production callers.) |
| `projection` | **(file) projection backend**: turn skills/commands/instructions/agents into on-disk surfaces (junction/symlink/materialized-copy/atomic-swap placement) for each agent. |
| `gal-engine` | Workflow CLI primitives: canonical-root render, CLI types (`CommandKind`/`ExitCode`/`Action`/`classify_args`), gal-self `install`, workflow `doctor`, `git_filter` registration, `translation`. Hosts internal binary subcommands including the `*-check` family (`finalize-check`, `pipeline-converge-check`, `boundary-check`, `pipeline-preflight`, `planning-check`, `prompt-check`, `refining-check`; see D13-D15), `restore`, and `marketplace-snapshot`. |
| `cli` | The `gal` binary entry; subcommand dispatch + `gal doctor` aggregation of all `HealthCheck`s; the gal workflow command surface. |

> Naming follows [`docs/naming.md`](naming.md): no generic bucket names (`projection` not
> `adapters`, `mcp::serializers` not `providers`); `executor` not `dispatch::adapters`.

## What gal is — and is not

gal is a **cross-agent workflow product**. It authors one workflow interface (the `/gal` commands,
golem agents, skills, gal's own MCP servers) and **projects it into every supported coding agent**
(Claude, Copilot, Codex, Antigravity, Gemini, opencode). The projection engine (`projection` + `mcp`
+ `gal-engine`'s render) and the workflow runtime (`dispatch` + `pipeline`) are the whole
product.

**Out of scope (moved to the ccync product):** managing *third-party* plugins/MCP — the catalog,
adopt/reconcile, machine-level install orchestration, MCP-host aggregation, and the
`backup`/`restore`/`uninstall`/`release` machinery. gal keeps the projection *engine* (to project its
own face) but ships none of the third-party-management surface.

## Key architectural decisions

### D1 — Canonical data structure = the Claude plugin format

gal's **canonical internal data model is the Claude Code plugin format** (`.claude-plugin/plugin.json`
+ the marketplace schema). gal renders its own content (gal-core, plus the optional machine-local
personal layer) once into this canonical form at `~/.gal/plugins/gal`, then projects it to each agent.
One canonical model in → many agent projections out.

- **Recall** (`docs/naming.md`): a **plugin** is the *distribution container*; a **skill** (SKILL.md,
  the cross-tool Agent Skills open standard) is the *content*.

### D2 — Project at the open-standards layer, not per-vendor code plugins

gal projects **SKILL.md skills + MCP servers + AGENTS.md/instructions + agents** — the cross-tool open
standards that all supported coding agents consume, including opencode. gal does **not** generate or
run opencode's TypeScript *code* plugins (the only out-of-scope extension layer).

### D5 — Runtime-host components are consumers, not part of the plugin

A component built specifically to let **one** runtime use gal fully — e.g. a VS Code extension that
surfaces gal commands inside Copilot Chat — is a **runtime consumer**, not part of the gal plugin. It
is placed **surface-namespaced**, never inside the canonical plugin root and never at a repo-root
`extensions/`:

- machine: `~/.gal/<surface>/…` (e.g. `~/.gal/vscode/extension/`)
- repo source: a parallel surface-namespaced directory mirroring the machine layout

**Forbidden**: `~/.gal/plugins/gal/…` (the canonical plugin root) and a repo-root `extensions/`. The
canonical plugin root holds only the **projected plugin** (skills, commands, agents, manifests, per
D1); a host that *reads* that projection lives **beside** it, not inside it. gal is **skill-only**
today — no such component ships; this decision binds any future one.

### D6 — Multi-source projection: gal-core baseline + machine-local personal layer

The skill/agent projection layer supports an **ordered source list** (`Vec<ProjectionSource>`) rather
than a single root:

| Type | id | `persistent` | Role |
|---|---|---|---|
| **core** | `"gal-core"` | `true` | Baseline; always first; never pruned |
| **personal** | `"local"` | `true` | Machine-local owner layer (`~/.gal/local/`), presence-based: directory/file exists → projected, absent → render is byte-identical to a Core-only render. No config flag gates this. |

**Collision policy (core-wins + additive-only + warn):** gal-core is the fixed baseline. A non-core
source may only contribute names not yet claimed; a clash skips the item and emits a
`ProjectionWarning`. No silent overwrite.

**Cross-source prune safety:** `prune_stale_links` receives the full installed source id set; an item
is pruned only when its `owning_source_id` (recorded in the lockfile's `sourceAttribution` map) is
confirmed absent from the installed set, never merely absent from one invocation.

> The bundle and personal-CC-plugin *management* sources (catalog-driven) are a ccync concern and are
> not part of gal.

### D7 — Cross-runtime role invocation: agent projection is format-aware, not verbatim copy

Golem agents are authored as Claude-format `*.agent.md` source files (D1), but the runtime-facing
surfaces are **format-aware**:

| Target | Format | Notes |
|---|---|---|
| **Claude** | `agents/<name>.md` (**Claude MD subagent**) | `gal-engine/src/render/manifests.rs::filter_agent_for_claude`; frontmatter filtered to Claude keys, abstract tool tokens expanded. Not a verbatim copy. |
| **Codex** | `~/.codex/agents/<name>.toml` (**Codex TOML**) | `projection/src/codex_agent.rs`; `developer_instructions` = charter body; `sandbox_mode` from invocation mode. |
| **Antigravity** | `agy-agents/<name>.agent.md` (**agy native**) | Verbatim native Claude-format agent file; Antigravity reads the native `.agent.md` lane. |
| **Copilot** | `~/.copilot/agents/<name>.agent.md` | `.agent.md` shape, abstract tool tokens rewritten to Claude names — also format-aware, not verbatim. |

**Tool name mapping has a single source of truth** (`projection/src/tool_map.rs`): abstract tool
permissions (`read/edit/execute/web`) map to valid runtime names per runtime. Invocation mode
(consult vs build) controls permissiveness (Codex `read-only` vs `workspace-write`).

**Consult dual-mode:** `/gal <role>` → isolated (only verdict returns); `/gal discuss <role>` →
in-context (activation-core loaded, multi-turn). **Orchestrated-only exclusion:**
`{implementer, tester, auditor, researcher}` are excluded from native subagent projection but remain
in `KNOWN_GOLEMS` (pipeline dispatch resolves them); a bare `/gal <role>` without orchestration
context returns unknown-intent.

### D8 — Release is its own lane, not a phase of pipeline or finalize

**Decision:** release is a first-class `release-<slug>` plan type, not a phase inside `/gal pipeline`
or `/gal finalize`. `golem-releaser` is the fourth planning-stage consult role (alongside architect,
analyst, designer) and is consult-only: abstract tools = `read/search/web`, no `edit`/`execute`.

**Lifecycle:**
1. `/gal releaser` (isolated consult) — researches release APIs/CI tooling, designs the release/devops
   flow, emits design advice. Does not write files. Does not execute.
2. `/planning release-<slug>` — materializes the design advice into a `release-<slug>.md` source plan.
3. Normal `/gal pipeline` — golem-implementer/tester/auditor execute the plan atomically.
4. `/gal finalize` — lands and closes the release plan like any other plan.

**Why not inside finalize?** Finalize's Sequence 2 is doc-sync + lifecycle only. Release tooling
(`gal release` binary subcommand: checksums, artifact-manifest, winget/homebrew manifests) is part
of the release plan execution, not the finalize gate. This keeps finalize a universal landing step
and prevents its scope from expanding with each new release target.

**Pipeline `Phase` enum unchanged:** `{Implement, Test, Audit}` only — no `Phase::Release` was added
and no `VERIFY`/`RELEASE` routing key exists in `config.json#executorRouting`. The `gal release`
binary subcommand (`CommandKind::Release`) is release-internal (like `finalize-check`/`migrate`),
not a public `/gal` control-plane command.

### D9 — CLI command surface = workflow + task dispatch; lifecycle = update / init / doctor

**Decision:** `gal`'s public command surface is partitioned into two concerns:

- **Workflow**: `dispatch`, `pipeline`, `commit-msg`, `dispatch-script`
- **Lifecycle**: `update` (version + upgrade instructions), `init` (repo adapter generation), `doctor` (health check)
- **Internal subcommands** (not public `/gal` surface): `clean`, `smudge`, `naming-gate`, `finalize-check`, `pipeline-log`, `release`, `refresh`, `restore`, `marketplace-snapshot`, `translation-freshness`, and the planning/pipeline `*-check` family

The `install`, `sync`, and `setup` commands were **removed**. Binary installation is handled by package managers (cargo/winget/Homebrew/curl); repo-adapter generation is `gal init`; git-filter registration is manual (`git config filter.gal-config.clean 'gal clean'` etc.).

**Why:** The projection pipeline (`run_machine_update`, skill/agent symlink management, canonical-root render triggered by `gal install`/`gal sync`) was a management concern that blurred the workflow-product boundary. Removing it eliminates ~800 lines of machine-state mutation code from the `gal` binary, leaves `render_canonical_root_from` as a pub fn used by `gal init`, `gal refresh`, and `finalize-check`, and makes the on-machine lifecycle entirely driven by the user's own package manager. This decision was recorded here so the rationale survives plan-file deletion.

**Remaining surface (24 `CommandKind` variants):** `Update`, `Doctor`, `Dispatch`, `Pipeline`, `CommitMsg`, `Clean`, `Smudge`, `Init`, `TranslationFreshness`, `DispatchScript`, `NamingGate`, `FinalizeCheck`, `PipelineLog`, `Release`, `Refresh`, `PipelineConvergeCheck`, `BoundaryCheck`, `PipelinePreflight`, `PlanningCheck`, `PromptCheck`, `RefiningCheck`, `PlanningStamp`, `Restore`, `MarketplaceSnapshot`. (`Xmachine` retired — see D22.)

`gal refresh` is an internal binary subcommand added after D9 as the machine-level restore/rebuild primitive for derived content. See **D11** below.

The three `*-check` variants (`PipelineConvergeCheck`, `BoundaryCheck`, `PipelinePreflight`) are pipeline-internal binary subcommands added for the pipeline guard mechanization. See **D13** below.

### D10 — Source-root resolution = cwd-walk → packaged layout; no dev-mode toggle

**Decision:** The `gal` binary locates its source root by a `.git`-bounded cwd-walk, falling through to a binary-side packaged layout. The `GalMode`/`devMode`/`galRoot` configuration infrastructure was **removed**; `GalConfig` is now an empty forward-compatible struct (`config_path()`/`load()`/`load_from_path()` retained). Retired `devMode`/`galRoot` keys still present in `config.json` are silently ignored, and `gal doctor` emits a non-blocking warning for them.

**Why:** Mode-based source resolution required the user to configure which layout they were in; the environment already encodes that fact (a checkout has `.git`; an installed binary ships a packaged layout). Resolving automatically removes a config surface and a class of "wrong mode" misconfiguration. Recorded here so the rationale survives plan-file deletion.

### D11 — `gal refresh` is the machine-level restore/rebuild primitive for derived content

**Decision:** `gal refresh` (`CommandKind::Refresh`) is an internal binary subcommand (not part of the public 11-command `/gal` surface) that re-orchestrates existing pub fns (`render_canonical_root_from` + `machine_skill_options` + `run_update_skills` + `run_update_commands`) to rebuild:

1. canonical plugin root (`~/.gal/plugins/gal`)
2. parent Claude marketplace manifest (`~/.gal/plugins/.claude-plugin/marketplace.json`)
3. per-runtime projections for all 5 runtimes (copilot/codex/agy/gemini/opencode)

**Boundary:** `gal refresh` writes only GAL's own `~/.gal` tree. It does **not** write `~/.claude/plugins/cache` or `installed_plugins.json` — those are Claude-managed. Claude re-picks up the updated plugin after restart or re-add.

**Why:** After D9 removed `gal install`/`gal sync`, there was no CLI entry point to rebuild derived content after a binary update or accidental deletion. `gal refresh` fills that gap as a minimal rebuild primitive without reviving the deleted management machinery or adding new authority. It re-uses existing pub fns rather than reviving the deleted `run_machine_update` orchestrator. All-5-runtime projection is the current design because there is no active `install-state.json` writer post-D9 (F2, documented in `docs/manual.md`).

### D12 — `golem-releaser` always asks first; deploy capability resolved from read-only sources

**Decision:** `golem-releaser` confirms the deploy target with the user before any research or design (always-ask-first, regardless of whether project context could infer the target). After confirmation it resolves locally-available deploy capabilities across four read-only surfaces: MCP manifests, skills directories, API web-research, and CLI config files. Deploy target authority is H-class — the user decides the target, releaser never self-assumes.

**Scope:** dual-mode — GAL-self CLI binary release (GitHub Releases + package managers) and downstream repo deploys (web service / backend / firebase-class / npm / Docker). Scope stops at target-driven design advice; `golem-releaser` does not claim a deploy orchestrator identity (no canary, rollback, or production monitoring).

**Why zero code (route a):** MCP/skills sources are deterministic files; API research uses the existing `web` abstract tool; the CLI gap is covered by the always-ask user exchange. A new read-only subcommand would re-implement file reads in new code (minimalism Q4) and tripping `crates/projection/` Protected Path for zero functional gain. Route (a) — zero code, no Protected Path, no new crate dependency, deterministic fact sourcing.

**Honest reporting:** a read-miss means the capability is not available — releaser reports it explicitly. No fabrication. No PATH execution-probe (read-only constraint preserved).

### D13 — Pipeline guard mechanization: Three-Zone Principle for per-task checkpoints

**Decision:** Three pipeline enforcement points (Step-1 prerequisites, 2c pre-commit allowlist, 2g convergence) are mechanized as pipeline-internal binary subcommands (`gal pipeline-preflight`, `gal boundary-check`, `gal pipeline-converge-check`). Each produces a machine receipt; the receipt is the sole pass-basis for the corresponding SKILL gate — self-report is not accepted. `not-run` always exits non-zero and is treated as fail.

**Three-Zone Principle:** read-only verification → mechanized binary (receipt-based); semantic judgment stays AI (orchestrator correctness gate, goal-backward verify); destructive actions stay orchestrator-exclusive (commit boundary). The three `*-check` binaries occupy only the first zone — they verify, they do not judge semantics or execute commits.

**Scope boundary:** `## Affected Files` missing or empty → `boundary-check` exits `not-run` (non-zero). This is an invariant of the binary's judgment body, not just a test note — it prevents forgetting the allowlist from silently bypassing the boundary gate.

**Code surface:** three new `CommandKind` variants (`PipelineConvergeCheck`, `BoundaryCheck`, `PipelinePreflight`); all three are internal subcommands (like `finalize-check`, `naming-gate`, `pipeline-log`) — not part of the public `/gal` 11-command surface. The check primitives (`Receipt`, `CheckState`, `CheckOutcome`, `check_three_surface`, `checked_task_ids`, `commit_note_hash`) are shared `pub(crate)` fns from `crates/cli/src/commands/finalize_check.rs` (promoted by plan `refactor-finalize-check-shared-primitives`).

**Why:** pipeline per-task checkpoints (`2c allowlist`, `Step-1 prerequisites`, `2g three-surface convergence`) were pure AI prose before this plan; under DEGRADED_SAME_RUNTIME dispatch the orchestrator sometimes skipped them with no deniable record. The mechanized binaries mirror the pattern already proven in `/gal finalize` (finalize-check receipt gate), applying zero-trust verification symmetrically to the three pipeline per-task enforcement points.

### D14 — `gal finalize-check` extended to 8 checks; verify/destructive boundary enforced in-binary

**Decision:** `gal finalize-check` is extended from 5 to 8 checks by appending 3 read-only informational checks (append-only; existing 5 checks + all tests byte-identical):

- **check(f) `finalize-mode`** — `git branch --show-current` → `worktree` | `already-on-main`; state always `Pass` (informational, never a gate condition). `/gal finalize` Step 1 reads the `summary` field to determine whether merge + teardown apply.
- **check(g) `executor-log-scan`** — walks `.dev/executor-logs/*.log`, reads `terminal_state` header; any value ≠ `completed` → `Fail`; no logs → `NotRun`. Mechanizes the pipeline-integrity "no-receipt/timeout are never PASS" assertion.
- **check(h) `durable-layer-commit`** — scans `[x]` task lines for the last `*(hex)*` hash; `git cat-file -e` existence check. Summary: `exists: <hash>` / `missing: <hash>` / `not yet recorded`. Receipt states existence only.

**Verify/destructive boundary (enforced in-binary):** check(h) asserts existence only — it does NOT authorize or trigger plan-file deletion. Deletion stays orchestrator-executed after ORCHESTRATOR reads the receipt and confirms the hash independently. `Pass` in check(h) is necessary but not sufficient for deletion: STEWARD must signal with the durable-layer commit hash and ORCHESTRATOR must verify the hash matches Seq 2 before proceeding.

**Why append-only:** the existing 5 checks and their test oracle are foundational contracts referenced by all active finalize procedures; byte-level stability was required. The 3 new checks occupy the informational + "existence evidence" tier — they can safely run in any checkout state and never block on their own state alone.

### D15 — Planning-shell mechanization completes the internal `*-check` family

**Decision:** the planning-stage deterministic shell is enforced by three additional internal subcommands: `gal planning-check`, `gal prompt-check`, and `gal refining-check`. Together with `finalize-check`, `pipeline-converge-check`, `boundary-check`, and `pipeline-preflight`, they form one consistent receipt-driven `*-check` family: deterministic structure lives in the binary, judgment stays with AI.

**Scope split:**
- `planning-check` gates planning/deep-planning handoff (`OQ=None`, required sections, `ARCH_REVIEW: CLEAR`, naming-gate, `planLanguage` consistency)
- `prompt-check` gates compressed execution prompts (machine-anchor preservation only; no transform or rewrite)
- `refining-check` gates refined source-plan structure (task count, T/TP pairing, ID well-formedness, referenced paths, `ENG_REVIEW: CLEAR`)

**Scope:** `refining-check` also guards zero-task source plans (new lower bound: `task_count == 0 → Fail`), closing a gap where an empty `## Tasks` section silently passed the check.

**Why:** the planning flow previously relied on prose-only compliance. These three binaries mechanize only the deterministic shell and intentionally avoid Rust ownership of semantic compression, task design, or OQ judgment.

### D16 — Adversarial review method is single-source; role lenses stay local

**Decision:** the adversarial review method is centralized in `plugins/gal-core/skills/adversarial-review/SKILL.md` and referenced pointer-only from consult review roles and other review-facing surfaces. The skill owns the cross-role method: steel-man first, refute-by-default under doubt, minimalism by pointer, evidence discipline, explicit `APPROVE`/`REVISE`/`REJECT` verdicts, jidoka stop-line behavior, and the rule that `NotRun` is never pass.

**Boundary:** the skill does **not** flatten the per-role lens. Architecture dimensions stay in `golem-architect`; business dimensions stay in `golem-analyst`; UX dimensions stay in `golem-designer`; documentation-structure activation points stay in `golem-steward`; test-contract and verification philosophy stay in `golem-tester`. Shared method is centralized; domain judgment remains local to each role.

**Discuss propagation:** `projection::codex_agent::generate_discuss_skill_md` injects only the `<role>` section into `discuss-<role>` skills. Therefore the pointer must live in `<role>` rather than only in `<reference-appendix>` if `/gal discuss <role>` is expected to carry the shared review method in-context.

**Why:** the prior state duplicated adversarial method prose across multiple agents, drifted over time, and failed to reach in-context discuss because that mode injected activation-core only. Centralizing the method removes prose drift without weakening role specialization and preserves the 11-command public `/gal` surface because the change lives at the skill layer, not the command layer.

### D17 — Named Workflow Obedience: enforcement surface is AGENTS preamble + skill descriptions, not workflow prose

**Decision:** obedience to named GAL workflows (`$gal-pipeline`, `$gal-finalize`, `$gal-status`, `$deep-planning`, `$refining-plan`, `$plan-to-prompt`) is enforced at two always-on surfaces:

1. **AGENTS.md preamble** (`## Adapter Rules`) — the Named Workflow Obedience rule is rendered by `crates/cli/src/gal/render.rs` as the **first bullet in the preamble array** so it appears within the first bytes of AGENTS.md (< 4 KiB; verified by a unit test against the 32 KiB Codex read limit).
2. **Command-skill frontmatter descriptions** — explicit `$skill-name` invocation triggers and key EN/ZH activation terms are front-loaded in the first ~200 chars of each command description so implicit skill routing can resolve on partial reads.

**Not the enforcement surface:** prose inside workflow skill bodies (e.g., rules in `workflows/coding.md`) — these are loaded *after* a runtime has already chosen to enter the workflow, so they cannot fix "never entered the door" failures.

**Machine gate:** `gal pipeline-preflight` receipt must pass before any implementation edit. Multi-plan requests must expand to an ordered run list. Both are enforced by the Entry Latch block in `commands/gal-pipeline/SKILL.template.md` and by the `gal pipeline-preflight` binary check.

**Honesty boundary:** AGENTS.md rules improve salience but cannot guarantee obedience from a non-compliant runtime. Codex is documented as a bounded executor fallback, not a guaranteed orchestrator, when interactive orchestration fails.

### D18 — Marketplace snapshot is render-only, core-only, and separate from `gal refresh`

**Decision:** `gal marketplace-snapshot --source <dir> --out <dir>` is a publisher-internal binary subcommand that renders GAL's public plugin tree to an explicit output directory and writes the Claude marketplace catalog there. It calls `render_canonical_root_to(source, out/gal, include_machine_local=false)`, so the snapshot contains core GAL content only: no personal layer, no machine-local MCP, no host `bin/` binary, no runtime projection, and no `~/.gal` writes.

**Why separate from `gal refresh`:** `gal refresh` rebuilds a local machine's canonical root and projects to runtime surfaces. Publishing needs only a portable plugin snapshot. Reusing refresh would add unrelated projection side effects and would depend on home-directory semantics (`USERPROFILE` on Windows, `HOME` on Unix). The explicit `--out` path keeps CI publishing deterministic and platform-neutral.

**Install model:** the marketplace plugin is a discovery and instruction carrier, not a complete GAL install by itself. The `install-gal` skill is model-invocable and tells the AI to show the exact package-manager command, obtain consent, run Homebrew / winget / `cargo install --git` as available, and verify `gal --version`. No `/gal-install`, SessionStart, or GAL-side install state machine was added.

### D19 — Codex shared-skill-surface boundary: materialized copy, not junction; lockfile is the deletion-cause ground truth

**Decision:** `~/.agents/skills` is Codex's official user-skill root **and** a namespace shared with other skill-managing tooling (confirmed on-machine via a co-resident `vercel-labs/skills`-family lockfile). GAL's projection into that shared root writes a **materialized real-directory copy** for every core skill (`materialize_skill_dir`, idempotent per-file byte-compare) — the same mechanism GAL's command-skills already used — and **never places a junction/symlink there**. `gal doctor`'s `SkillsProjectionHealthCheck` treats the projection lockfile (`plugins.lock.json`) as ground truth for a missing-skill deletion discriminator: still lockfile-attributed → `external deletion (last GAL projection: <mtime>)`; absent from the lockfile → `not projected / pruned`; no lockfile → `unknown` (never a guessed cause). A shared-root entry GAL owns with no active command/skill/discuss-skill counterpart is a zombie, cleaned by `gal refresh` via `remove_gal_command_skill`. GAL-private namespaces it also writes to (`~/.copilot/gal`, the Gemini/Antigravity `gal` link) are **untouched** by this decision — they are not co-managed by third parties and stayed junctions.

**Why:** On-machine evidence (2026-07-02) showed the shared root's junction-based GAL entries being selectively deleted by something other than GAL while GAL's own private-namespace junctions and the shared root's real-file command-skill entries survived — a three-way contrast pointing directly at projection *strategy* (junction vs. real file), not at the deleter's identity, which was investigated and left unresolved. A junction dies in a namespace other tooling audits/clears; a real file survives it. This mirrors D11 and D18's "own tree only" discipline: GAL still writes read-only-verifiable, filesystem-observable state, but the shared root now degrades to stale-and-recoverable rather than wiped.

**Two real ownership-check bugs surfaced and fixed while building this**, both variations of the same class: lockfile attribution is keyed differently depending on how a projected item is written (`write_command_skill` marks the `SKILL.md` file path; `materialize_skill_dir`'s caller marks the directory path itself). `remove_gal_command_skill` and `SkillsProjectionHealthCheck::check_one`'s collision check both originally checked only one of the two shapes; both now check both. Recorded here because the fix generalizes: any future projection-ownership check touching this shared root must check both attribution shapes, not just the one its author happened to test against.

### D20 — Non-English planning: three-layer authority, inline equivalence stamp, deterministic/semantic split

**Decision:** When `planLanguage != en`, planning uses three explicit authorities instead of one file: an **EN semantic draft** (`.dev/plans/<slug>.en.md`, the sole planning-stage semantic authority; not a source plan), a **localized source plan** (`.dev/plans/<slug>.md`, the human-facing + GAL-tool-visible source/approval/review surface, rendered from the draft and carrying a planning-authority metadata block), and the **English execution prompt** (post-prompt authority, unchanged). For an `en`-prefix `planLanguage` there is no draft — the source plan is its own EN authority (single-file fast path). A hand-edit to the localized source is reconciled back into the EN draft (diff-base = `render(EN draft)`, no third snapshot) before any downstream planning-stage command continues; after `/plan-to-prompt`, the EN draft is deleted and the equivalence proof — that the English prompt is an equivalent rendering of the now-deleted draft — persists as the source plan's own inline `prompt-hash` / `equivalence-verdict` metadata fields, stamped in place by `gal planning-stamp --equivalence <prompt>`. There is no separate `.equiv.md` receipt file (a `refactor-fold-equivalence-receipt-into-source` correction — see below); a non-English plan has exactly two files after `/plan-to-prompt`, not three. The rendered-source hash excludes the metadata block itself (no self-reference stale) and normalizes LF + Unicode NFC + trailing-space strip for cross-machine (Windows CRLF) stability.

**Deterministic/semantic split:** the Rust check layer (`planning-check`, `prompt-check`, and the internal `gal planning-stamp` write-side, sharing one `planning_authority` helper module) does only deterministic work — metadata parse, normalized hashing, rendered-source-hash freshness, machine-anchored parity (task/test IDs + file paths, never prose), language heuristic, and the inline equivalence verdict + a live re-hash of the prompt body. The semantic merge of a localized edit back into the EN draft is the planning-stage command's (model's) job, never Rust; the gates never degrade to prose-only reminders and parity never over-promises on cross-language prose (prose equivalence is model-judged and recorded in the inline verdict).

**Why (original three-layer decision):** owner real-usage evidence showed pure non-English planning loses technical semantics across sessions and Codex edits (Codex reverts localized sections to full en-US, especially file-list sections). An EN semantic authority is the tested need. The language gate uses a pure-English-narrative-line fraction rather than a raw Latin-letter ratio: a genuine technical zh-TW plan sits near 0.76 Latin at the letter level (dense inline English terms), so a letter-ratio gate false-positives the very plans it protects; the line-fraction signal (real zh-TW ≈ 0.05) catches a section reverting to English without that false positive. Machine anchors (headings, paths, IDs, verdict literals, metadata keys) stay English (keep, no rename) because `prompt-check`/`finalize-check` depend on them.

**Why (inline stamp, not a sibling receipt file):** the original design made the equivalence receipt a tracked sibling file (`.dev/plans/<slug>.equiv.md`) so a fresh checkout kept the audit evidence outside gitignored `.dev/pipeline/`. In practice its only real job was satisfying `prompt-check`'s equivalence gate, and it duplicated hashes the source plan's `gal:planning-authority` block already carried — the block is the correct existing home for the proof. Folding it inline (`prompt-hash` + `equivalence-verdict` fields, six-field schema) removes a whole file class with no loss of audit durability: the source plan itself is the tracked artifact. The one-time migration that retired the field upgraded every live five-field block in place and folded any live `.equiv.md` into its source plan before the legacy reader was removed; the three plan-token classifiers (`commit_msg.rs`, `dispatch_script/model.rs`, `commands/dispatch.rs`) still exclude a stray `.equiv.md` suffix as a documented defensive guard against a legacy or downstream orphan file, even though nothing writes one anymore.

### D21 — Language convention supply chain: three sources, presence-based, read-only detection

**Decision:** GAL Core ships zero owner-personal coding-style conventions — `plugins/gal-core/conventions/` carries only `rust.md` (GAL's own dev standard). Language house style reaches a downstream repo through three sources instead:

1. **Personal convention files** (`~/.gal/local/conventions/<lang>.md`) — user-written, per-repo always-on. The repo-adapter selector matches a file's stem against the project's `Language` row using the same word-boundary tokenizer as the core selector (aliases `c#`/`.net`→`csharp`, `javascript`→`typescript`, `golang`→`go`; any other stem is generic/universal and always matches).
2. **Installed agent-plugin detection** (Detected Language Skills) — read-only. GAL scans installed skill roots and renders a reference block (name + origin + a standing load-first instruction) when a skill's name/origin tokens match the Language row. GAL never copies, vendors, manages, or deletes the plugin's content.
3. **Personal skills** (`~/.gal/local/skills/`) — on-demand, loaded by the agent when named.

**Flag removal — placement is opt-in, double opt-in is an anti-pattern:** the personal layer used to require both a directory *and* a `personalLayer.enabled` config flag (`personal_layer_enabled`, deleted). That double gate meant placing a file was not enough to activate the layer — a second, easy-to-forget config edit was also required. The fix collapses the gate to presence-based: a file/directory present at the personal root is itself the opt-in, matching how every other machine-local GAL surface already behaves. Marketplace-snapshot exclusion is carried entirely by `include_machine_local=false` (D18), which is orthogonal to this flag and was never affected by its removal.

**Personal conventions slot (source 1):** lives in the non-Protected cli render layer (`crates/cli/src/gal/render.rs`), not gal-engine — zero new config keys or crates. Token matching is shared with the core selector via one `language_tokens` function, so word-boundary semantics (no Django/MongoDB false-matching `go`) are identical across both sources. A repo-level `| Personal Conventions | off |` row in `.dev/project.md`'s Tech Stack table is GAL's own self-protection mechanism (its own repo carries it) — it disables sources 1 and 2 for that repo so a public repo's tracked adapters never embed owner-machine content; source 3 is unaffected since it is never injected always-on.

**Installed-plugin detection (source 2):** strictly read-only and reference-only — the injected standing instruction is GAL's strongest guarantee, but actual loading remains the agent's job, documented as an explicit honesty boundary. Matching is restricted to a skill's name + origin-path tokens; description text is never matched (a false-positive generator for short tokens like `go`). Names failing a safe charset (alphanumerics, `-`, `_`, bounded length) are rejected before ever reaching a rendered instruction surface — closing a prompt-injection-via-third-party-skill-name vector. Staleness (a newly installed plugin not yet detected) is resolved by rerunning `gal init`, not by any background watcher.

### D22 — Remote execution is an SSH spawn lane inside `dispatch`, not a separate `xmachine` crate

**Decision:** Cross-machine task execution is a second spawn lane composed inside `dispatch::run` — when a `config.json#executorRouting` role entry carries `sshTarget` + `remoteWorkdir`, the same resolved invocation (executor + model + args + stdin spec) that runs locally runs instead as `ssh <target> "cd <remote-workdir> && <executor> <args>"`, over a synchronous held session. The prior `xmachine` crate (remote `gal pipeline` inside `zellij`, `status.json` polling, `result.patch` pull) and its unwired `pipeline::orchestration` `Transport`/`LocalTransport` scaffold (zero production callers once `xmachine::ssh::SshTransport` was gone) were both retired, not adopted.

**Why:** The live dispatch path already resolves a route and spawns a process (`dispatch::run::run_dispatch` → `dispatch::dispatch::spawn_executor`); a remote branch inside that same composition is the smallest change that puts remote execution on the live path. `zellij`/polling/patch-pull was heavier than the synchronous spawn model dispatch already uses, and remote runs the raw agent CLI directly — no `gal` needed on the remote host. See `docs/remote-execution.md` for the config keys, remote prerequisites, and safety boundaries (the remote workdir is a GAL-dedicated checkout; post-apply cleanup is destructive by design).

**Boundary (v1):** POSIX remote only (Windows remote deferred); copilot is local-only (its CLI-flag spec delivery cannot be safely forwarded over ssh); no disconnect-survival; the remote lane is single-dispatch / manually-re-synced — an autonomous multi-task remote pipeline that self-advances the remote HEAD after each control-node commit is a deferred follow-up.

### D23 — Codex GAL pipeline reliability is a diagnostic lane, not more agent prose

**Decision:** Codex-specific pipeline-execution failures are turned into durable, machine-checkable diagnostics across three separated layers rather than into more instruction text inside `$gal-pipeline`. **(1) Prompt/tool surface:** the Named Workflow Obedience bullet plus a compact **GAL critical runtime pack** are rendered by `crates/cli/src/gal/render.rs` into the AGENTS.md preamble within Codex's `project_doc_max_bytes` budget (marker-offset tests, not full-adapter loading). **(2) Projection/runtime surface:** `SkillsProjectionHealthCheck` gained a canonical-source freshness comparison (a projected `~/.agents/skills/<name>/SKILL.md` differing from canonical content — resolved across the flat, `skills/<name>`, and `commands/<name>` layouts — is reported stale → `gal refresh`); `gal doctor` also reports dev-checkout binary/source skew (baked `GAL_GIT_STAMP` vs GAL-checkout HEAD), Codex skill-budget top contributors, and Codex Memories/`[mcp_servers.*]` as advisory-only. **(3) Pipeline-execution surface:** the `gal-pipeline` / `gal` SKILL templates carry PowerShell-safe `'#file:<prompt>'` quoting, explicit multi-plan prompt selection, and a sandbox-write-denial hard-stop; `dispatch-script` drops the residual `verify` phase and the missing-prompt error explains the PowerShell comment pitfall; a Codex nested-executor readiness classifier degrades on `codex exec` arg0/PATH-alias access-denied.

**Why:** The observed 2026-07-09 failures (stale skill, stale binary, unquoted `#file:` stripped by PowerShell, sandbox `reason=log-error` write denial, generic-pipeline-block mistaken for phase execution, multi-plan ambiguity, adapter/skill budget pressure, disabled Memories wrongly blamed, missing MCP graph tools, retired-tool warnings, nested access-denied) each degraded silently because Codex re-derived nothing from a durable surface. A model cannot enforce "trust no self-report"; the fix is diagnostics + compact rendering + preserved authority (ORCHESTRATOR owns preflight/phase-sequencing/in-process goal-backward verify; Codex Memories/MCP/same-runtime role-play get no new authority). Docs-only would have left the same silent-failure modes.

### D24 — `.dev/project.md` is a bounded current-topic index enforced at the render write-boundary

**Decision:** `.dev/project.md`'s `## Verified Facts` is a fixed current-topic index (ten topics: runtime/layout, lifecycle/control plane, adapters, documentation/structure map, planning/review/gates, dispatch/remote, naming/personalization/Core, install/release/restore, Codex compatibility, research), each topic one bullet stating only its current state with a durable `README.md`/`docs/` pointer. Updating a topic is **upsert/replace/prune** — never append. The single authority for this shape, the mutation model, and the size budget lives in `plugins/gal-core/conventions/token-budget.md` § Bounded Current-Topic Index; `docs/manual.md` and `docs/devguide.md` reference it rather than restating it.

**Render-choke enforcement:** `crates/cli/src/gal/render.rs` validates `.dev/project.md`'s normalized-LF UTF-8 byte size against a hard 30,720-byte cap immediately after reading it, before any of the five adapters are rendered or written. A violation rejects the whole render with **no partial output** — none of the five adapters are touched — rather than writing a partial or truncated set. This is a genuine choke point: every repo-adapter render path (`gal init`, `gal refresh`, `/gal finalize`'s doc-sync) reads `.dev/project.md` through this one function.

**Discoverability ordering is unrelated to D17.** The same render pass also reorders `## Repo Skills` to precede `.dev/project.md` content for the three carriers that carry a skill index (`CLAUDE.md`, `AGENTS.md`, `GEMINI.md`); `.github/copilot-instructions.md` and `.agents/rules/gal.md` continue to omit the section. This is a **skill-discoverability** change (where the skill list appears in the rendered file) and does **not** redefine or weaken **D17**'s Named Workflow Obedience enforcement surface (the AGENTS.md preamble ordering and skill-description front-loading) — the two concerns are orthogonal: D17 governs *whether* a runtime enters a named workflow, this decision governs *where the skill index sits relative to project content* inside an already-rendered adapter.

**Post-doc-sync sequencing:** at `/gal finalize`, STEWARD's `.dev/project.md` reindex must run before any adapter-render idempotency evidence is collected, since the reindex changes the content that flows verbatim into every adapter — a double render performed before the reindex cannot stand in as post-doc-sync evidence. See `plugins/gal-core/commands/gal-finalize/SKILL.template.md` Sequence 2.

**Why:** `.dev/project.md` had grown into an unbounded per-plan history sink (65 `## Verified Facts` entries plus 12 more misfiled under `## Documentation Gaps`), regularly exceeding the render budget every downstream adapter re-reads. A render-time choke point — rather than a convention-only rule — makes the bound structurally enforced rather than merely documented.

## See also

- [`docs/naming.md`](naming.md) — naming authority (terms, casing, reserved words).
- `docs/devguide.md` — maintainer ops, install topology, making changes.
- `docs/getting-started.md` — install → init → first plan.
- `docs/remote-execution.md` — SSH remote-execution lane: config keys, prerequisites, safety boundaries.
