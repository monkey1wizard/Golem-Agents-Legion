# Developer Guide

**How to change GAL** — maintainer procedures only: the dev inner loop, change procedures, and machine operations. **Developer-only.** This document assumes a source checkout and an intent to modify GAL. What the system *is* lives in [`docs/architecture.md`](architecture.md), which is diagram-first and written for any reader including non-developers. Contributors start at [`CONTRIBUTING.md`](../CONTRIBUTING.md). Document responsibility boundaries for the whole repo are defined in [README → Documentation](../README.md#documentation).

If you cannot tell which layer you are touching, stop and resolve that first — read [architecture → Codebase Ownership Map](architecture.md#codebase-ownership-map). Most broken refactors in GAL come from mixing README, docs, templates, and command contracts in one change. This guide assumes a source checkout ([contributor setup in the README](../README.md#working-on-gal)). The `gal` binary resolves the source root automatically (cwd-walk → packaged layout, [Source-Root Resolution](architecture.md#source-root-resolution)).

## Dev Inner Loop (edit → see it live)

A source edit in `plugins/gal-core/` does **not** reach the coding agents on its own. Nothing auto-deploys after a commit. To make a change actually run:

| You changed... | Steps to see it live |
| --- | --- |
| `plugins/gal-core/` content only (SKILL/agent/command/conventions) | `gal refresh --source ./plugins/gal-core` |
| `crates/` (any Rust) | rebuild (`cargo build --release -p gal-cli`) → copy the binary onto `PATH` (`cp target/release/gal.exe ~/.cargo/bin/gal.exe`) → `gal refresh --source ./plugins/gal-core` |

`gal refresh` re-renders the canonical root and re-projects all five runtimes. The new SKILLs reference whatever subcommands the binary exposes. A stale `PATH` binary makes the projected SKILL call commands that return `unknown command`. Always update the binary **before** refreshing when `crates/` changed. **Claude alone needs a restart** to reload. The other four runtimes pick up the refreshed skills on next invocation.

`gal refresh` accepts `--source <path>` to point at an explicit source root. On a dev checkout, always pass `--source ./plugins/gal-core`. Without it, `gal refresh` tries to locate source beside the binary (packaged flat layout) which fails from `~/.cargo/bin`.

**Two recurring traps:**

- **Missing `--source` on a dev checkout.** Running bare `gal refresh` from `~/.cargo/bin` fails to locate source because there are no `skills/`+`agents/`+`commands/` dirs beside that binary. Always pass `--source ./plugins/gal-core` when running from a dev checkout.
- **`preserved user-owned path` warnings = a skipped projection.** Pre-lockfile stale projections (no GAL header, not in the attribution lockfile) are protected by the projection fail-safe and won't be overwritten. Confirm the named dir is GAL content (frontmatter `name: gal-*` / a known command). Remove it and re-run `gal refresh --source ./plugins/gal-core`. The second run reports 0 warnings.

**Freshness diagnostics:** `gal doctor` catches stale session contracts:

- **Stale skill projection** — a projected `~/.agents/skills/<name>/SKILL.md` (skill or command-projected skill like `gal-pipeline`) whose content differs from the current canonical source is reported stale. Run `gal refresh` then restart Codex or open a fresh thread. A forgotten refresh after editing a SKILL template is a visible doctor finding.
- **Binary/source skew** — when the `gal` binary on PATH was built from an older commit than the GAL-checkout HEAD (`GAL_GIT_STAMP` ≠ `git rev-parse --short HEAD`), doctor recommends rebuild and reinstall. This highlights editing `crates/` without reinstalling the binary. It is distinct from a stale skill. A fresh skill projected by a stale binary still calls the binary's older subcommands.

Rule of thumb: **`plugins/gal-core/` change → `gal refresh`. `crates/` change → rebuild + reinstall the binary *then* `gal refresh`.** Both require a Codex restart or fresh thread (Claude: restart) to reload.

## Making Changes

### Rules & Fences

#### 1. Markdown owns the durable contract

- Methodology, rules, and contracts live in tracked Markdown and source files.
- Generated adapters, baked command files, and runtime configs are outputs, not source inputs.

#### 2. `/gal` only solves control-plane problems

- `/gal` should not wrap a second copy of tester, auditor, designer, debugger, or releaser work. `golem-releaser` is a planning-stage advisory role. It is not a pipeline execution role. It runs before implementation, not after.
- Execution-stage specialist behavior belongs in agents.
- Planning commands run directly as part of the public command surface.

#### 3. Repo-local state is the ownership boundary

- `.dev/`, `.dev/plans/`, `docs/designs/`, and similar repo-local files are the shared working state.
- Do not move GAL's core state back into user-global storage.

#### 4. Missing tools must not look like success

- GAL uses skill-level routing. It avoids one repo-wide CLI-first or MCP-first rule.
- Each external-tool skill defines a preferred path, a fallback path, and a no-tool behavior.

#### 5. Do not optimize one runtime by breaking portability

- Changes causing Copilot, Antigravity, and Codex to diverge in contract or file flow are incorrect.
- README, docs, templates, and setup flows preserve cross-runtime parity first.

#### 6. Navigation docs must not become a second spec

- Keep navigation documents short and directional.
- Summaries are useful. Duplicate contracts are not.

#### 7. Optional-capability routing belongs to the workflow layer

- Agents must not silently switch personas or contracts upon detecting an optional capability.
- The workflow decides the capability first. The capability writes back into the same repo-owned files.

The shared five-state preflight model lives in [../plugins/gal-core/conventions/optional-capabilities.md](../plugins/gal-core/conventions/optional-capabilities.md).

### Before Editing Install

1. Decide which concern is changing: bootstrap payload, machine setup, provider plugin lifecycle, repo init, generated adapters, or documentation copy.
2. Read the primary owner from [architecture → Owning Surfaces](architecture.md#owning-surfaces). Read only the directly called code or docs for that concern.
3. For contributor builds, the source root resolves via cwd-walk. No config change is needed.
4. Preserve user-owned machine intent during upgrade and default uninstall.
5. If the change affects public install wording, update English and the translated copies under `docs/i18n/<lang>/` together, or explicitly mark the translation drift.
6. Run the narrowest install tests covering the changed lane: `cargo test --workspace` + `gal doctor`.

### Common Change Entry Points

#### Changing `/gal` or alias behavior

1. Read [../plugins/gal-core/commands/commands.md](../plugins/gal-core/commands/commands.md).
2. Check whether the change is contract-level behavior or only install/runtime presentation.
3. If it affects generated command files, inspect the relevant `commands/*/SKILL.template.md`.

#### Adding or changing a planning command

1. Place it in the right family via [../plugins/gal-core/commands/commands.md](../plugins/gal-core/commands/commands.md).
2. Update the owning prompt in `commands/<command>/SKILL.template.md`.
3. Confirm the write-back target fits the existing plan sections and workflow state machine.
4. If it changes optional-capability semantics, update [../plugins/gal-core/conventions/optional-capabilities.md](../plugins/gal-core/conventions/optional-capabilities.md).

#### Adding or changing an execution specialist

1. Update the owning prompt in `agents/<golem>.agent.md`.
2. Confirm the write-back target fits the existing plan sections and workflow lifecycle.
3. Update [../plugins/gal-core/agents/agents.md](../plugins/gal-core/agents/agents.md), [../plugins/gal-core/commands/commands.md](../plugins/gal-core/commands/commands.md), and README sections routing users to that specialist.
4. Do not reintroduce the behavior as a standalone public command unless it is control-plane or planning work.

#### `/gal finalize` — plan completion landing (vs `/gal wrap-up`)

`/gal finalize` is the control-plane command landing and closing a completed plan. It runs only after `/gal pipeline` finishes every task and the orchestrator's end-of-run goal-backward verification returns VERIFIED. It is a thin orchestrator with zero new authority. It performs a hard precondition gate, whole-branch holistic review (delegated to `golem-architect` for correctness/architecture/scope and `golem-auditor` standalone branch-audit for perf/security). On a worktree/feature branch, it performs a plain `git merge` to main plus `git worktree remove`/`git branch -d` teardown. It triggers doc-sync (STEWARD knowledge → `docs/`) and delegated lifecycle close (ORCHESTRATOR marks ABSORBED and deletes the plan files). The canonical contract lives in [../plugins/gal-core/workflows/coding.md](../plugins/gal-core/workflows/coding.md) → Plan Finalization. This guide does not redraw the flowchart.

**finalize vs wrap-up — keep these distinct:**

| | `/gal finalize` | `/gal wrap-up` |
| --- | --- | --- |
| Purpose | Completion **landing** (land + close a done plan) | Session **pause** (hand off mid-flight work) |
| When | All tasks `[x]` + goal-backward VERIFIED | Anytime, at any plan state |
| Effect | One-shot, gated, **destructive** (merge + plan-file deletion) | **Non-destructive** (handoff notes + session continuity) |
| Reverse-prompt | Offers `/gal finalize` once when the plan is complete | Offers `/gal wrap-up` if the user is pausing |

A maintainer changing finalize behavior edits `commands/gal-finalize/SKILL.template.md` (the command contract) and `workflows/coding.md` (the canonical flow + delegation boundary). Re-sync per the closeout section below.

#### `gal finalize-check` repo-hygiene rows — maintainer notes

The four repo-hygiene checks live in `crates/cli/src/commands/finalize_hygiene.rs`, wired into `finalize_check.rs::cmd_finalize_check`. Policy bodies are authoritative elsewhere (architecture § [Repo Hygiene](architecture.md#repo-hygiene)). The following covers implementation-boundary notes only:

- **Parser scope is deliberately bounded, not a full Markdown parser.** `doc-link-resolution`'s link scanner (`scan_markdown_links`) handles inline `[label](target)`, image `![alt](target)`, and reference-style `[label][ref]` + `[ref]: target`. It masks backtick-quoted inline-code spans and skips fenced ` ``` `/`~~~` blocks first. Anything outside that shape (nested links, HTML `<a>` tags, footnote syntax) is out of scope by design. Extend the fixture-locked test matrix before extending the scanner.
- **`git ls-files` pathspec quirk — `:(glob)` magic is load-bearing.** Git's default pathspec matching treats a bare `**` as requiring at least one full directory segment. The `docs/**/*.md` pattern alone silently drops direct children of `docs/`. `tracked_markdown_paths` prefixes both `**` patterns with `:(glob)` to restore "zero or more directories" semantics. Do not remove that prefix without re-verifying against a repo with direct-child docs.
- **Roster sources are the same three docs `contract-roster-parity` cross-checks.** Commands ↔ `commands/commands.md` § Source Files (backtick paths). Agents ↔ `agents/agents.md` § Agents And Responsibilities (markdown-link targets). Templates ↔ `templates/templates.md` § File Templates + § Other Files (unioned). The union must equal every direct file in `templates/`, including `templates.md`. Adding a new command/agent/template file requires adding its roster row in the same commit. Otherwise, `contract-roster-parity` fails on the next `gal finalize-check`.
- **Normalized-byte rules follow the `.dev/project.md` precedent.** `state-bound`'s 16,384 B cap is CRLF→LF normalized before measuring. The comparison is strictly `>` (exactly-16,384 B passes), matching `.dev/project.md`'s 30,720 B cap in `render.rs`. Keep any future per-file byte cap consistent with this normalize-then-strictly-greater-than pattern.
- **Receipt diagnostics: two modes, two default paths, one exact mode line.** `render_finalize_receipt` (in `finalize_check.rs`, not a method on the shared `Receipt` struct) renders `mode: full` or `mode: hygiene-only` verbatim. The `default_receipt_path(HygieneMode)` returns distinct filenames ensuring `--hygiene-only` runs never overwrite the full-mode evidence file or vice versa.
- **`Receipt` stays check-family-agnostic — do not add a `mode` field to it.** `Receipt`/`CheckOutcome`/`CheckState` are reused verbatim by six other check subcommands lacking a mode concept. `render_finalize_receipt(&Receipt, HygieneMode)` is a `finalize_check`-local free function. It follows the per-command custom-render pattern those six modules use. Always rebuild the whole `gal-cli` binary after touching `Receipt`.
- **Test fixtures are hermetic and plan-ID-literal-free.** Every hygiene-check test builds its own `TempDir` fixture instead of asserting against the repo's real files. A temporary `#[ignore]`d live-repo sanity check is a development aid and must be removed before commit. No plan-task ID appears as a literal in any fixture. Compose IDs at runtime if a test requires one.
- **Double-render procedure is unchanged.** See Regenerating Adapters and MCP below. The hygiene checks add no new render step.

#### Closeout: re-syncing a roster/role contract change to a live runtime

When a change adds, removes, or renames a golem persona or a dispatch routing role, the source edit in `plugins/gal-core/` does **not** reach the user's live `/gal` surface automatically. The running runtime still loads the previously-installed plugin payload. This is a **manual, machine-mutating closeout step**, run after merge by the maintainer. It is **not** an automated `/gal pipeline` task.

Procedure:

1. **Project source to the runtime:** run **`gal refresh`** to rebuild the canonical plugin root and re-project every runtime's skill/command surface. This step carries a `plugins/gal-core/` source edit to the live agents. `gal init --force` does **not** carry edits to agents. It only rebuilds repo-local adapters. See Dev Inner Loop for binary-staleness caveats.
2. **Regenerate repo-local adapters:** run `gal init --force` in each initialized repo to rebuild CLAUDE.md, AGENTS.md, and other adapter files from the merged source.
3. **Migrate live machine-local config:** if a routing role was renamed, or the config still uses the retired flat shape, update the `executorRouting` subtree inside `~/.gal/config/config.json` to the two-group form (`pipeline` / `planning`). A retired flat role key degrades gracefully. `gal dispatch` prints a retirement warning naming the key and its target group, then falls back to text/in-process dispatch until the key moves. There is no alias or silent fallback.
4. **Refresh the Claude plugin cache:** restart Claude Code (see Claude Plugin Cache Refresh). The other four runtimes pick up the refreshed skills on their next invocation.

Acceptance checklist (run on the target machine after re-sync):

- a removed/renamed golem no longer resolves. Example: `/gal old-golem` returns unknown-intent.
- the live agents surface contains no removed persona file.

#### Regenerating Adapters and MCP

- After changing `plugins/gal-core/mcp.json`, run `gal refresh` to re-render the canonical root and runtime MCP projection. Machine-local secret values still come from `~/.gal/config/config.json`. The `gal update` command is only a version-print stub. `gal refresh` handles projection.
- Run `gal init --force` after changing source-of-truth content requiring repo-local adapter regeneration such as `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, or `GEMINI.md`. Repo adapters and runtime projection are separate processes. `init --force` handles adapters. `gal refresh` handles the runtime skill/command surface.

#### Refactoring docs themselves

1. Ensure each doc has one clear job. [README → Documentation](../README.md#documentation) acts as the authority.
2. If another source file already owns the contract, summarize it and link out instead of copying it.
3. If you remove content from one reader entry point, provide a clear new landing page.

### Adding A New CLI Runtime

1. Decide whether the CLI has a machine-layer config directory GAL can target.
2. Decide whether its repo-facing instruction file can reuse `AGENTS.md` or needs another generated adapter.
3. If the runtime supports native commands, generate them from the shared command templates instead of building a second workflow source. Otherwise, install the baked command skills into the runtime's supported skill surface.
4. Add a config-merge bridge only if the runtime has a stable user-owned config file accepting additive changes safely.

### Verify & Self-Check

After changing install or setup logic, verify at least these points:

- the stable projection target exists for each supported runtime requiring one
- generated `commands/*/SKILL.md` files no longer contain `{{GAL_ROOT}}`
- Gemini native command files are regenerated from the baked command content
- Antigravity installed skills and agents resolve through the agy plugin root
- AGY plugin-root `mcp_config.json` serves as the sole GAL-managed MCP source for AGY without remaining GAL-managed MCP entries in the global `mcp_config.json` or `settings.json`
- shared skill directories contain reusable skills only without duplicated command aliases
- MCP reruns update tracked server entries correctly without clobbering unrelated provider-owned config

Before you finish a maintainer change, also ask:

- Did I create a second source of truth?
- Does `/gal` still only solve control-plane problems?
- Do specialist commands still write back to repo-owned files?
- Did I accidentally move state back into a user-global path?
- Can a missing tool still fail loudly instead of pretending to succeed?
- Did I keep optional-capability routing at the workflow layer?
- Do README, `commands/commands.md`, and this guide still have distinct jobs?

### Drift Queue

Known navigation/ownership drift to watch when editing install or release docs:

- Provider lifecycle status claims split across docs and code. Before changing public claims for Claude, Copilot, or Codex, compare [../README.md](../README.md), this guide, and [../crates/gal-engine/src/render/](../crates/gal-engine/src/render/).
- The translation freshness check flags translated copies under `docs/i18n/<lang>/` falling behind canonical sources. Resync before changing public install status claims.

## Operations

### Distribution

The public release-chain shape, the artifact layout contract, and the static-CRT Windows invariant are architectural decisions owned in full by [`docs/architecture.md`](architecture.md) — **[Release Lanes](architecture.md#release-lanes)** and **[Public Release Chains](architecture.md#public-release-chains)**. Release operation detail including exact remotes, credentials, tag procedure, per-stage failure handling, and rollback stays in a private maintainer-only operator runbook. It is never part of the public doc tree.

The `gal` binary distributes via package managers (cargo / winget / Homebrew / curl). After binary installation or reinstallation, run **`gal refresh`** to rebuild the canonical plugin root, the parent Claude marketplace manifest, and per-runtime projections from source. `gal refresh` avoids writing `~/.claude/plugins/cache` or `installed_plugins.json` because those remain Claude-managed. Claude picks up the updated plugin via restart or re-add (`claude plugin add ~/.gal/plugins`).

`gal init` generates repo-local adapters from the gal-core templates. `gal update` prints current version and upgrade instructions. `gal doctor` checks the local setup. Public release and package-manager distribution ship via `packaging/` and `.github/workflows/release.yml`.

### Source Restore

`gal restore [--yes]` reverts the GAL source repo to the last `/gal finalize`-verified known-good state including source and derived layers. It does not copy machine-local config across machines.

| Layer | What restore does |
| --- | --- |
| **Source** | `git reset --hard gal-last-good` + `git clean -fd` — the worktree **exactly** equals the marker commit. Tracked files added after the marker and all untracked files are removed. |
| **Derived** | Delegates `gal refresh` — rebuilds canonical root + runtime projections from the restored source. |

**`gal-last-good` marker.** `/gal finalize` writes a lightweight git tag `gal-last-good` to the landing commit. It is non-fatal. `gal restore` reads that tag as the **sole baseline**. An absent tag triggers a fail-closed response with an actionable message. It avoids HEAD fallbacks and guessing. The tag does not exist until the first `/gal finalize` after a fresh bootstrap. Until then `gal restore` refuses cleanly.

**Safety net.** Before any mutation, `gal restore` creates a `gal-restore-backup-<ts>` branch at the current HEAD. This operates as a **hard precondition**. If branch creation fails, the command aborts without touching the worktree. Nothing is silently lost.

**Confirmation guard.** Without `--yes`, the command prints the discard scope including uncommitted changes and commits ahead of the marker. It exits with a usage error. This forms the intentional dry-run path.

### Claude Plugin Cache Refresh

Claude Code copies the local directory-source plugin into a cache at `~/.claude/plugins/cache/gal/` and serves that copy. Because GAL's manifest carries **no** `version`, a stale cached copy persists after the content changes. **Restart Claude Code** after a refresh to pick up new commands. Fully automatic refresh-on-commit via a git-source marketplace defers to GAL's public release.

**When the cache is stale** (the canonical root rebuilds but Claude still reads the older cache):

1. Run `gal doctor` — it reports a cache-staleness warning when `~/.claude/plugins/cache/gal/` is older than `~/.gal/plugins/gal/` (`ClaudePluginCacheCheck`, non-blocking `Warning`).
2. Restart or reload Claude Code — it rebuilds the cache from the canonical root automatically on startup.
3. If the warning persists after restart, manually remove `~/.claude/plugins/cache/gal/` and restart Claude Code.

This situation commonly occurs after updating the binary and re-running `gal refresh` in a running Claude Code session.

## Documentation Conventions

This section serves as the authoritative naming, structure, and translation policy for the documentation tree. The directory `docs/i18n/<lang>/` holds only the translation and terminology-profile artifacts. This section acts as the single place the policy lives.

> **Document responsibility boundaries are owned by [README → Documentation](../README.md#documentation)**, not this section. That table is the single authority on which document holds which job, plus the contributor reading order. This section owns the layer below it: what counts as durable, how files are named, and how translations are tracked.

### Durable Documentation Layer (Single Authority)

This subsection is the **GAL-global sole authority** on what constitutes the durable knowledge sink. Other plans and conventions reference this definition without redefining it.

**Durable knowledge sink = `README.md` + `docs/`** (the entire `docs/` tree, at canonical EN paths).

**Excluded from the sink:**

- `.dev/plans/` — transient staging area. Plans are deleted after lifecycle closure.
- `.dev/research/` — investigation scratch. Promoted facts move to `docs/` or `.dev/plans/`.

**`.dev/project.md` is the compressed index of `docs/`, not a sink.** It summarizes and cross-references durable docs without owning facts. Durable knowledge is extracted *into* `docs/`, then indexed *from* `.dev/project.md`. This remains consistent with the Knowledge Flow in `conventions/token-budget.md`:

```text
Plan ## Status / ## Handoff Notes  →  docs/ (permanent)  →  .dev/project.md (index)
```

Implication: a fact surviving a plan's lifecycle closure must land in `README.md` or a non-excluded file under `docs/`. Leaving it only in `.dev/plans/`, `.dev/research/`, or `.dev/` is not durable.

**`.dev/project.md`'s re-index has its own bounded content shape.** Its `## Verified Facts` content shape, mutation model (upsert/replace/prune, never append), and hard size budget are owned by `plugins/gal-core/conventions/token-budget.md` § Bounded Current-Topic Index (see architecture § [Bounded Current-Topic Indexes](architecture.md#bounded-current-topic-indexes)). STEWARD's re-index step must follow that rule without restating it here.

### Punctuation: No Semicolons

**No document in this repo uses a semicolon** in any language. This includes both ASCII and full-width variants. A semicolon indicates the sentence carries two complete thoughts at once. Rewrite it instead. Split into two sentences when the halves stand alone. Use a comma plus an explicit connective (`and`, `but`, `so`, `because`, `while`) when one depends on the other. A list of parallel items separated by semicolons becomes a comma list or a bullet list.

The goal is readability, not uniformity. A semicolon hides the logical relation between its two halves and forces the reader to reconstruct it. The rule covers Markdown body text, table cells, and English annotations inside ASCII diagrams and code comments. Code requiring a semicolon as syntax is not affected.

### File Naming

- **`README.md` is reserved for the single repo-root README.** No other file in the repo may be named `README.md`. A sub-area entry/index doc uses `guide.md` (a curated signpost) or `index.md` (a generated or listing index) instead.
- **Tool/capability sections** in `docs/integrations.md` use the tool's own name. Only an MCP-server-specific section carries an `MCP` qualifier (`Playwright MCP`). A workflow or methodology doc never receives a tool-suffixed name.
- **Skill-aligned docs** match the skill name they support. The `opencli-research` skill is supported by the OpenCLI section of `docs/integrations.md`.
- No two unrelated docs may share the same basename.

### Multi-Language Translation Policy

The policy ensures the set of languages can grow without churn while the set of translated docs stays closed. It does not grow on demand.

- **Canonical = English** at the main filename in its normal location. Canonical docs are never moved for translation.
- **Translations live under `docs/i18n/<lang>/`** and are named `<name>.<lang>.md`. Both the folder and the filename carry the language to ensure a translation file is self-describing out of context. The basename is never bare `README.md` to honor the single-README rule.
- **Mirror-path rule**: root `README.md` → `docs/i18n/<lang>/README.<lang>.md` and `docs/X.md` → `docs/i18n/<lang>/X.<lang>.md` preserving sub-paths. The canonical doc's top carries a language-switch link to each translation. Each translation links back.
- **Closed public translation allowlist — exactly three documents**: `README.md`, `docs/manual.md`, `docs/integrations.md`. These form GAL's public user-entry surface. The allowlist does not grow by adding more docs. If a doc's audience is genuinely public-entry, that remains a policy decision rather than a docs-formatting one.
- **EN-only developer authorities (deliberate policy, not missing coverage)**: `docs/architecture.md`, `docs/devguide.md` (this file), `docs/naming.md`, `CONTRIBUTING.md`, everything under `plugins/gal-core/conventions/` and `plugins/gal-core/workflows/`, `.dev/plans/`, and `.dev/research/`. A contributor must read English for all of these regardless of their own locale. Translating them would create a second driftable copy of a development authority. A full translation of any of these is reported `unexpected` by `gal translation-freshness`.
- `<lang>` tags follow the existing convention in use (e.g. `zh-Hant`, `ja`). `zh-Hant` is supported. This means all three allowlisted translations plus the terminology profile exist.

### Two Translation Artifact Kinds

`docs/i18n/<lang>/` holds two distinct kinds of artifacts. Each carries its own freshness rule. Do not conflate them. A terminology profile is never treated as or checked like a full translation.

1. **Full translation** — a complete rendering of one of the three allowlisted canonical docs (`README.zh-Hant.md`, `manual.zh-Hant.md`, `integrations.zh-Hant.md`). Tracked `current` / `stale` / `missing` against its canonical source. A full translation of a doc **not** on the allowlist is reported `unexpected`. It is drift to be removed rather than a translation to keep fresh.
2. **Locale terminology profile** — one per supported locale, e.g. `docs/i18n/zh-Hant/terminology.zh-Hant.md`. It records how canonical English terms from `docs/naming.md` are presented to that locale's readers. It tracks staleness against `docs/naming.md`'s source commit like a full translation. It is not a translation of `docs/naming.md` and must never claim to be one. A terminology profile is identified by its exact filename `terminology.<lang>.md`. Any other file under `docs/i18n/` declaring `source: docs/naming.md` is a disguised full translation and is reported `unexpected`.

**A locale is "supported" only when both its three allowlisted full translations and its one terminology profile exist.** A locale directory with any of the four missing is incomplete rather than a different tier of support.

### Translation Freshness

Every translation artifact under `docs/i18n/` carries YAML front-matter to ensure drift against its English source is visible:

```yaml
---
source: README.md          # repo-relative path to the canonical source (docs/naming.md for a terminology profile)
lang: zh-Hant
source_commit: <hash>      # the source commit this artifact is in sync with — PENDING until stamped
translated_at: 2026-06-02
status: current | stale     # human hint — the check recomputes from source_commit
---
```

`gal translation-freshness` scans `docs/i18n/`, compares `source_commit` against `git log -1 --format=%H -- <source>`, and reports each `(doc, lang)` pair as:

- `current` — hash matches the source's current commit.
- `stale` — hash differs, is missing, or is a non-hash placeholder like `PENDING`.
- `missing` — one of the three allowlisted full translations or the terminology profile does not exist yet for a locale directory that otherwise exists.
- `unexpected` — a full translation exists for a doc that is not on the three-document allowlist.

It is report-only and scales linearly with languages and artifact kinds.

Operational notes:

- Because GAL commits are authored by a human after review, a freshly synced artifact is stamped `PENDING` until the maintainer records the real source commit after committing. The check surfaces those as `stale` ensuring they are not forgotten.
- A translation flagged `status: stale` is knowingly behind its English source. Such files are excluded from the hard repo-wide broken-link gate. The freshness check owns them until they are re-translated and re-stamped.

**Adding a translation:**

1. Pick one of the three allowlisted canonical docs.
2. Create `docs/i18n/<lang>/<name>.<lang>.md` at the mirrored path with the front-matter above using `source_commit: PENDING` until first commit.
3. Translate the body. Keep headings and anchors aligned with the source. Preserve all machine literals (commands, paths, config keys, product names) untranslated. Re-base every relative link for the translation's deeper location. `docs/i18n/<lang>/` sits two directory levels below the repo root. Canonical relative links copied verbatim all break. Root `README.md`'s `docs/manual.md` becomes `../../manual.md`.
4. Add a language-switch link at the top of the canonical doc and back from the translation.
5. After committing, re-stamp `source_commit` with `git log -1 --format=%H -- <source>` ensuring `gal translation-freshness` reads `current`.

**Adding a language:** create `docs/i18n/<lang>/`. Add the three allowlisted translations plus one terminology profile as detailed above. The locale is "supported" once all four exist. There is no partial-support tier and no schema or tooling change needed to add a language.

### Planning-Doc OQ Lifecycle

Open Questions in `.dev/plans/*.md` are scaffolding rather than a permanent record:

- When an OQ is resolved, bake the ruling into the section that owns it. This is typically a `## Decisions` table with the ruling, date, and who decided. Delete the OQ entry. Do not keep `- [x]` OQ corpses.
- When all of a plan's OQs are resolved, do a full rewrite pass of the plan instead of incremental patching. Incremental patches leave "see OQ-xx" cross-references pointing at deleted or moved content. This makes plans progressively unreadable.
- A plan body must never require the reader to reconstruct decision history from OQ archaeology. History belongs to git. The plan states only the current ruling.

### Planning-Doc Item IDs

Requirement, task, and test-point IDs in `.dev/plans/*.md` use two-digit numbering (`Rnn`, `T-nn`, `TP-nn`, `OQ-nn`). Never use three-digit zero-padding (`T-0nn`). Two digits acts as the cap. If a plan needs three-digit numbering for roughly 100+ tasks or test points, it is too large for one plan and must be split into multiple plans. A single source plan should stay well within two digits. A task count creeping toward that ceiling operates as a signal to decompose rather than to widen the ID format.

This forms a format rule for new and not-yet-implemented plans. Do not retroactively renumber an in-flight plan whose IDs are already referenced by commit messages, an execution prompt, or `.dev/executor-logs/`. The churn outweighs the consistency and the IDs constitute load-bearing history.

## Token Discipline

These rules apply to all maintainer and agent work in this repo. The full policy lives in [../plugins/gal-core/conventions/token-budget.md](../plugins/gal-core/conventions/token-budget.md). The developer-facing summary follows here.

### Generated-Artifact Exclusion

Do not read generated adapters (`CLAUDE.md`, `GEMINI.md`, `AGENTS.md`, `.github/copilot-instructions.md`) or build outputs (`bin/`, `obj/`) unless the current task is explicitly auditing those generated files. They are large and frequently regenerated. They contain no information not already in their source templates.

### Directed Exploration

Before reading any file, confirm it is named in the current task or is a direct dependency of a task-named file. Stop reading when you have the information needed. Do not load the full codebase as a cold-start step.

### Failure-Focused Output

When running builds or tests, emit:

- Build: first error with file and line reference. On success, output one summary line only.
- Tests: failing test names and assertion messages only. Do not echo passing test names.
- Lint: files and rule violations only. On a clean pass, output one summary line only.

Store full logs on disk when needed. Retrieve specific lines selectively rather than piping entire logs into context.

### Context-Pressure Recovery

When context is near the limit during an active task:

1. Write the current task name, last completed step, and any key decisions to `### Handoff Notes` in the active plan's `## Status` section.
2. Write or update the matching plan row in `.dev/state.md` `## Session Continuity` with `Stopped At` and `Next Step`.
3. Do not create a separate `CONTEXT.md` file. The plan and state files act as the only durable session state stores.

This ensures the next session can resume without re-deriving context.
