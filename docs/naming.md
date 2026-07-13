# GAL Naming Authority

> **Single source of truth** for what every core term in GAL means and how names are chosen.
> When a name in code, comments, docs, or plans is ambiguous, this file decides.
>
> - Canonical language: `en` (per `PROJECT_LANGUAGE`). A `zh-Hant` translation lives under
>   `docs/i18n/zh-Hant/naming.zh-Hant.md`.
> - Code identifiers, file paths, command names, and product names are literal — never translated.
> - `docs/architecture.md` describes **structure** (the crate DAG / dependency law) and links here
>   for **vocabulary**. This file does not duplicate the DAG.
> - Owned by the `infra-naming-authority` plan. Core terms are anchored to the most neutral
>   authority that covers them — standards bodies (ISO/IEC, NIST) and the Linux Foundation's
>   Agentic AI Foundation (which governs MCP + AGENTS.md, the two standards GAL implements) —
>   before vendor sources. See [authority tiers](#part-b--core-external-concepts-authority-anchored)
>   and [Sources](#sources).
>
> **Glossary conventions.** The term tables in Parts B–D follow the **arc42 §12 "Glossary"**
> structure (a single project-wide term registry) extended with **Domain-Driven Design
> "Ubiquitous Language"** discipline: every term carries a **Status** (`Validated` = current
> canonical term · `Deprecated` = retired, do not use) and **Aliases** (accepted synonyms and
> the wrong names it must not be confused with). Retired terms are also emitted in a
> [machine-readable block](#retired-terms-gate-input) consumed by the naming gate.

## How to use this document

1. Before introducing a name that could mean more than one thing, find it here. If it is not
   registered, register it (Naming Logic Rule 6) before using it.
2. Code comments and durable docs state **durable intent**, never **provenance**
   (see Rule 4) — plan-task IDs and migration history do not belong in shipped artifacts.
3. When two parts of GAL disagree on a term, this file wins; fix the divergent side.

---

## Part A — Naming Logic (the rules)

These are the project-specific rules by which names are coined and disambiguated.

1. **One term, one meaning.** A noun maps to exactly one concept project-wide. If a concept needs
   a name and the obvious word is already taken, coin a qualified name — do not reuse the bare word.
2. **Reserved bare words, qualified everything else.** Two bare words are *reserved* to one GAL
   meaning and may be used unqualified:
   - **`agent`** → **golem agent** (GAL's own specialist AI-agent personas). This is a deliberate
     product-design choice: GAL's agents are golem agents, so plain "agent" means them.
   - **`model`** → **LLM** (the brain). "model" and "LLM" are fixed synonyms in GAL.

   Every *other* concept that an English word like `agent`/`adapter`/`surface` could grab must be
   qualified: an external AI coding tool is always a **`coding agent`** (never bare "agent"),
   plus **`MCP server`**, **`(file) projection backend`**, **`generated runtime adapter`**,
   **`runtime surface`**. Bare use of an unreserved overloaded word is forbidden in code
   identifiers and durable docs.
3. **Entities vs roles.** The same external **coding agent** is named by the *role it plays on an
   axis*: a `runtime` when it is the target of adapter generation, an `executor` when it is spawned
   for a dispatch stage, an `MCP host` when it consumes MCP servers. Never use one role-name on
   another axis, and never call a coding agent a bare "agent" (that word is golem agent's).
4. **Provenance lives in plan memory, not in durable artifacts.** Plan-task IDs
   (`T-NN`, `R-NN`, `TP-NN`, `FU-NN`, `BUG-X`) and migration narration ("repointed at…",
   "strangler seam", "until then…") are *provenance*. They are allowed only in `.dev/**`. Shipped code, comments, and durable docs state *durable intent* only.
5. **Names reflect the current owner, not migration history.** A module/file/symbol name must
   describe its present role/crate (e.g. `install_orchestration`, not `legacy_plugins`).
6. **Register new overloaded-prone terms here before use**, with their axis and owner.
7. **Identifier formation (house style).** How a multi-word identifier is *built*:
   - **Precedence — common standard first, house style second.** The language's common standard is
     authoritative and wins on any conflict: for Rust that is the
     [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/naming.html) + RFC 430
     (casing, trait/constructor conventions, acronyms). This GAL house style applies **only** where
     the standard leaves a free choice — word selection, word order, headword form — and **never**
     overrides the standard.
   - **Data** (types, fields, variables, modules) = **(verb→adjective) + Noun** — a verb turned
     into a participle/gerund adjective modifying a head noun: `CodingAgent`, `DefinedName`,
     `ResolvedRoot`, `GeneratedAdapter`.
   - **Functions / methods** = **verb-led action phrase** (the verb stays a verb):
     `define_name`, `resolve_runtime`, `generate_adapter` — *not* the adjective form
     (`defined_name()` ✗, that names the result, not the action).
   - **Canonical headword = `UpperCamelCase`, no underscores** (`CodingAgent`). That is the form
     registered in this file and used as the type name. **Casing then defers to the language /
     item-kind standard** — for Rust, RFC 430: `snake_case` for fields, locals, modules, and
     functions (`coding_agent`), `SCREAMING_SNAKE_CASE` for consts. This rule fixes only *word
     choice, word order, and the headword form* — never the casing, which the language owns.
8. **No generic bucket names.** Do not name a crate/module/file with a catch-all plural
   container word — `adapters`, `services`, `providers`, `utils`, `helpers`, `common`,
   `managers`, `handlers`, `misc`. They become junk drawers and re-create the overload that
   Rule 1 forbids. Name a unit by its **single responsibility**: `projection` (not `adapters`),
   `executors` (not `dispatch::adapters`), `mcp::serializers` (not `providers`). If such a
   generic word is genuinely unavoidable, there must be **at most one** in the whole project and
   it must be registered here with its scope.
9. **No accidental name duplication; standard slots are the only exception.** Aim for a folder
   name, file name, or function name to occur **once** in the project, so a search jumps to a
   single place. Do not reuse one identifier in different locations for different things.
   **Exception — names fixed by an external standard or a deliberate GAL convention are *meant*
   to repeat**, because each occurrence is the same structural slot (not an overload):
   - Cargo layout (repeats once **per crate**): `src/`, `src/lib.rs`, `src/main.rs`, `mod.rs`,
     `Cargo.toml`, `tests/`, `build.rs`.
   - GAL command standard (repeats once **per command dir**): `SKILL.md`, `SKILL.template.md`.
   - Trait / constructor method names (protocol slots, not clashes): `new`, `default`, `from_*`,
     `to_*`, `as_*`, `into_*`, `iter`, `fmt`, and any method implementing a shared trait.
   - Per-runtime adapter standard filenames: `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`,
     `.github/copilot-instructions.md` (one per repo; literal external tokens).

   A repeat that is **not** a recognized standard slot is a Rule-1 / Rule-8 violation — rename it.
   (Note: `README.md` is *not* a repeatable slot here — GAL policy is exactly **one** canonical
   `README.md`; translations use `README.<lang>.md`.)

> **Authority for casing:** Rust follows the [Rust API Guidelines — Naming](https://rust-lang.github.io/api-guidelines/naming.html)
> (C-CASE, built on RFC 430): types/traits/enum-variants `UpperCamelCase`; functions/methods/vars/
> modules/crates `snake_case`; consts/statics `SCREAMING_SNAKE_CASE`; acronyms are one word
> (`Uuid`, not `UUID`). See `plugins/gal-core/conventions/rust.md`.

---

## Part B — Core external concepts (authority-anchored)

These definitions are fixed to authoritative sources. They are the vocabulary GAL borrows from the
wider ecosystem; do not redefine them locally.

**Authority tiers** (prefer the most neutral that covers the term):

1. **Standards bodies** (neutral, formal) — ISO/IEC JTC 1/SC 42, NIST. Best for the genus *AI agent* and core AI terminology.
2. **Linux Foundation — Agentic AI Foundation (AAIF)** (neutral steward since 2025-12-09) — governs the two open standards GAL implements: **MCP** and **AGENTS.md** (plus goose). Higher authority than any single vendor.
3. **Vendor first-party** — Anthropic / OpenAI / Google. Authoritative for *their own product names*; treat category claims as marketing unless echoed by tier 1–2.
4. **Encyclopedic** — Wikipedia. Neutral but tertiary.

| Term | Status | Aliases / a.k.a. | Canonical definition | Authority | Do **not** confuse with |
| --- | --- | --- | --- | --- | --- |
| **AI agent** (genus) | Validated | intelligent agent | An entity that perceives, decides, and acts toward goals; defined as formal AI terminology. | **ISO/IEC 22989:2022** (defines "AI agent" among 110+ terms); NIST AI Agent Standards Initiative | a chatbot; an LLM. (In GAL, bare **`agent`** = golem agent — Part C; both golem agent and coding agent are *instances* of this genus.) |
| **model** / **LLM** | Validated | LLM (fixed synonym) | A neural network trained on a vast amount of text for language tasks. The "brain". | Wikipedia, *Large language model*; ISO/IEC 22989 ("model", "AI system") | the agent or chatbot built on top of it |
| **chatbot** / **AI assistant** | Validated | AI assistant | A consumer-facing conversational app built on an LLM (ChatGPT, Claude, Gemini *the apps*). | Wikipedia | the LLM itself; a coding agent |
| **coding agent** | Validated | agentic coding tool; agentic development platform | An AI agent that reads a codebase, edits files, runs commands, and integrates with dev tools. The category GAL targets. | **Neutral**: Linux Foundation **AAIF** defines AGENTS.md as guidance for "**AI coding agents**". **Vendor**: Anthropic "agentic coding tool" (Claude Code), OpenAI "coding agent" (Codex), Google "agentic development platform" (Antigravity). | the model it runs on; the surface it runs in |
| **surface** / **interface** | Validated | interface | *Where* a coding agent runs: terminal/CLI, IDE, desktop app, web. One agent can have several. | Anthropic: Claude Code "available in your terminal, IDE, desktop app, and browser" | the agent itself ("CLI" ≠ the agent's category) |
| **MCP** | Validated | Model Context Protocol | Model Context Protocol — "the universal standard protocol for connecting AI models to tools, data and applications". | **Linux Foundation AAIF** (steward since 2025-12-09; donated by Anthropic); spec at modelcontextprotocol.io | — |
| **MCP host** | Validated | (the agent's MCP role) | The AI application that coordinates MCP clients (a coding agent **is** an MCP host). | MCP spec, *Architecture* (names Claude Code as the host example) | MCP server; ~~MCP provider~~ (no such role) |
| **MCP client** | Validated | — | The component inside the host that holds one connection to one MCP server. | MCP spec | the host or the server |
| **MCP server** | Validated | the provider | A program that **provides** tools/resources/context to clients. The *server* is the provider. | MCP spec | the host; ~~"MCP provider"~~ meaning the agent (backwards) |
| **AGENTS.md standard** | Validated | — | "A simple, universal standard that gives **AI coding agents** a consistent source of project-specific guidance." GAL generates AGENTS.md alongside per-agent variants. | **Linux Foundation AAIF** (donated by OpenAI, 2025-12-09) | a GAL-specific invention |
| ~~**MCP provider**~~ | **Deprecated** | → use **MCP host** + **per-runtime MCP server config** | (retired) A drafting-era name for "a coding agent we write MCP config for". Backwards: the agent is the *host*, the server is the *provider*. | retired by `infra-naming-authority` | — (see retired-terms gate input) |

> **Retired term:** `MCP provider` (previously used in GAL drafts to mean "a coding agent we write
> MCP config for") is **wrong** and removed. The agent is the **MCP host**; the **server** is the
> provider. The correct GAL concept is *per-runtime MCP server configuration* (see Part C).

---

## Part C — GAL-internal vocabulary (how the external concepts map into GAL)

This is where GAL's own identifiers are pinned to the external concepts above.

> The Rule 8 crate/module renames (`adapters`→`projection`; `providers` folded into
> `mcp::serializers`; `dispatch::adapters`→`executors`) **landed** in `refactor-rust-architecture`
> (VERIFIED + closed 2026-06-14). Owner cells below name the **current** crate/module.

| GAL term | Status | Aliases | = which concept | Why this name / axis | Owner |
| --- | --- | --- | --- | --- | --- |
| **agent** (bare) = **golem agent** | Validated | golem agent (reserved bare `agent`) | a GAL specialist AI-agent persona (golem-architect, golem-tester, …) | **Reserved word.** Plain "agent" in GAL means a golem agent — a deliberate product-design choice. A golem agent is a genuine AI agent (ISO genus); it is a *role* that a coding agent performs. **Not** a coding agent, runtime, or executor. | `plugins/gal-core/agents/` |
| **runtime** | Validated | (coding agent, generation axis) | a **coding agent** GAL targets | GAL's code handle for a coding agent. Stays `runtime` (never bare `agent`) because **`agent` is reserved for golem agent**. Axis: *target of adapter generation*. | `base::runtime` (`VALID_RUNTIMES`) |
| **executor** | Validated | (coding agent, dispatch axis) | a **coding agent** invoked headlessly for a dispatch stage | Axis: *who runs this task*. Same entity as a runtime, different role. | `crates/dispatch` |
| **model** = **LLM** | Validated | LLM (reserved) | the LLM "brain" a coding agent uses | **Reserved word.** In GAL, "model" is fixed to mean the LLM and nothing else (e.g. `claude-opus-4-8`). Selected by executor-routing. | `config.json#executorRouting` |
| **per-runtime MCP server config** | Validated | (replaces ~~MCP provider~~) | the MCP **host** configuration written for each runtime | Replaces the wrong "MCP provider". GAL serializes each runtime's MCP host config (the agent is the host; it consumes servers). Subset: 5 of 6 runtimes (no `gemini`). | `crates/mcp` (`mcp::serializers`) |
| **projection surface** (surface) | Validated | surface | where a runtime sees GAL content on a machine | Axis: *where content lands* (skill / command / machine surface). | `crates/projection` |
| **projection** | Validated | — | the act of materializing canonical content onto a surface | junction / symlink / copy / atomic-swap. | `crates/projection` |
| **generated runtime adapter** | Validated | runtime adapter | the instruction file generated for a runtime to read | Product, not a code module. See member table for the file per runtime. | `crates/projection` + `gal-engine::render` |
| **(file) projection backend** | Validated | — | the code that turns skills/commands into surface files | Axis: *code role*. Not an "executor adapter". | `crates/projection` |
| **canonical root** | Validated | — | the rendered superset plugin root | `~/.gal/plugins/gal/`; the install product. | `gal-engine::render` |
| **source root** | Validated | — | the GAL source directory resolved by cwd-walk | Located automatically; no `galRoot`/`devMode` config needed. | `crates/cli::render` |
| **sync / install / setup** | Validated | — | repo-local adapters / canonical-root render / machine surfaces | Three distinct command boundaries (see `docs/devguide.md`). | `projection` / `gal-engine` / `setup` |
| **source plan / execution prompt / state** | Validated | — | `.dev/plans/<slug>.md` / `.dev/plans/<slug>.prompt.md` / `.dev/state.md` | The three plan-memory layers (see `plugins/gal-core/workflows/coding.md`). | repo |
| **dispatch / pipeline** | Validated | — | executor spawn (local + SSH remote lane) / local orchestration | The orchestration DAG: `dispatch ← pipeline`. Remote (SSH) execution is a spawn lane composed inside `dispatch::run`, not a separate crate (the retired `xmachine` crate + `pipeline::orchestration` scaffold are gone). | same-named crates |
| **local-first** | Validated | repo-first retrieval | the Core retrieval principle | Search codebase, docs, and repo-owned state before any optional external note backend. | `plugins/gal-core/skills/local-first-search`, `plugins/gal-core/workflows/research.md` |
| **local-notes** | Validated | external-note backend contract | the optional collaborative-tool lane for user-owned notes | App-agnostic, default-off, machine-local note backend contract; never a Core prerequisite. | `docs/collaborative-tools/local-notes.md` |
| **Personal Enhancement** | Validated | personal layer | machine-local opt-in behavior layered on Core | Owner- or user-specific workflows, paths, or rituals that must stay default-off and outside the Core contract. | `plugins/gal-core/conventions/core-vs-personal.md` |
| **Obsidian backend** | Validated | Obsidian integration | one possible `local-notes` backend | Optional app-specific backend; not the Core default and not the naming anchor for the portable note contract. | `docs/collaborative-tools/local-notes.md` |
| **plan-task ID** | Validated | T/R/TP/FU/BUG | `T/R/TP/FU/BUG` identifiers | *Provenance.* Allowed only in `.dev/**` (Rule 4). | `.dev` |
| **secrets** | Validated | — | `config.json#secrets` — credential key-value map | UPPER_SNAKE key names; values are secret-bearing strings (API keys, tokens). **Not** `vars`. The only secret-bearing section in `config.json`. | `crates/mcp::resolver`, `git_filters::run_clean_inner` |
| **executorRouting** | Validated | (replaces ~~executor-routing.json~~) | `config.json#executorRouting` — role routing grouped by consumer into `pipeline` + `planning` (plus the shared `executors` default-model block) | Sole routing source. Roles are grouped **by consumer**, not flat: `pipeline` (dispatch offload) and `planning` (projection consult). The retired flat shape (a role key directly under `executorRouting`) is not parsed — it warns by name. A `pipeline` role entry may additionally carry `sshTarget`/`remoteWorkdir` to route that phase over the SSH remote-execution lane (see `docs/remote-execution.md`); the standalone `executor-routing.json` is retired (no fallback). | `crates/dispatch::routing` |
| **pipeline** (group) | Validated | — | `config.json#executorRouting.pipeline` — dispatch-role group | Closed allowlist `{CODER, TESTER, AUDITOR}`; flattened into the dispatch table (`RoutingTable::get`, phase offload, SSH lane). A wrong-group or unknown role is skipped with a warning. **Not** the CI/CD sense of "pipeline". | `crates/dispatch::routing` |
| **planning** (group) | Validated | — | `config.json#executorRouting.planning` — consult-role group | Closed allowlist `{ARCHITECT, ANALYST, DESIGNER, RELEASER}`; validated for diagnostics but **never entered** into the dispatch table — read by `resolve_codex_routing` (projection) to resolve the Codex native-subagent model/`effort`. | `crates/projection::resolve_codex_routing` |
| **_galProjection** | Validated | — | top-level key in `plugins.lock.json` for the GAL projection registry | Written by `projection::persist`. Carries `agentProjectionPaths`/`commandProjectionPaths`/`skillProjectionPaths`/`legacyProjectionPaths`/`sourceAttribution`. **Never modified by the resolver.** | `crates/projection::support` |

> **Settled (2026-06-18, `c1-infra-config-consolidation`; updated `refactor-ssh-transport-replace-xmachine`):** `~/.gal/config/config.json` is the sole machine-config source (GAL-core machine settings + `secrets` + `executorRouting`, including per-role `sshTarget`/`remoteWorkdir`), with no fallback. The standalone files `executor-routing.json`, `config.local.env`, `mcp.local.json`, and `xmachine.json`/`xmachine.config.json` are all retired and no longer read (the `xmachine` remote-execution subsystem and its work-node config were replaced by the SSH dispatch lane on `executorRouting`). `plugins.lock.json` is mixed-ownership: the resolver writes its namespace; `crates/projection` writes `_galProjection`.

> **Settled (2026-06-14):** the code handle stays **`runtime`** (not renamed to `coding_agent`).
> Bare `agent` is **reserved for golem agent** by product design, so the external handle can never
> be a bare `agent`; `runtime` wins on zero churn over the qualified `coding_agent`. Canonical
> external term = **`coding agent`** (always qualified); internal handle = **`runtime`**.

---

## Retired Terms (gate input)

Machine-readable list consumed by the naming gate (`crates/gal-engine/src/naming_gate.rs`, introduced
by the `infra-naming-authority` plan). Each non-comment line between the markers is a **retired phrase**
that must not appear in durable surfaces. Durable = everything except provenance homes (`.dev/**`), the naming definition sites (this file, its translations, `conventions/naming.md`,
`naming_gate.rs`), and derived carriers regenerated from those sources (`*.private.md`, baked
`commands/<name>/SKILL.md`, and the generated repo adapters `CLAUDE.md`/`AGENTS.md`/`GEMINI.md`/
`.github/copilot-instructions.md`/`.agents/rules/gal.md`).
Match is case-insensitive, whole-phrase. Plan-task ID provenance (`T-NN`, `R-NN`, `TP-NN`, `FU-NN`,
`BUG-X`) is enforced by a separate regex in the gate, not listed here.

```text
# RETIRED-TERMS v1 — one retired phrase per line; lines starting with `#` and blank lines are ignored.
# Format is stable: the gate reads only this fenced block, by these BEGIN/END comment fences.
# RETIRED-TERMS:BEGIN
MCP provider
# RETIRED-TERMS:END
```

---

## Part D — Member registry (the coding agents GAL targets)

The six `VALID_RUNTIMES` are all **coding agents**, not models. Canonical preference order matches
`base::runtime::VALID_RUNTIMES`.

| runtime key | Coding agent (product) | Vendor | Generated adapter file | per-runtime MCP config? |
| --- | --- | --- | --- | --- |
| `copilot` | GitHub Copilot | GitHub / Microsoft | `.github/copilot-instructions.md` | yes |
| `antigravity` (`agy`) | Google Antigravity | Google | `.agents/rules/gal.md` | yes |
| `gemini` | Gemini CLI | Google | `GEMINI.md` | **no** |
| `codex` | OpenAI Codex | OpenAI | `AGENTS.md` | yes |
| `opencode` | opencode | open source (SST) | `AGENTS.md` | yes |
| `claude` | Claude Code | Anthropic | `CLAUDE.md` | yes |

> **Watch item (2026-06-13):** Google is transitioning **Gemini CLI → Antigravity CLI**; consumer
> access to Gemini CLI is scheduled to change **2026-06-18**. GAL lists both `gemini` and
> `antigravity` as runtimes — revisit whether they merge once the transition lands.

---

## Sources

Authority anchors for Part B, by tier (verified 2026-06-13).

**Tier 1 — Standards bodies (neutral, formal):**

- ISO/IEC 22989:2022 — *Artificial intelligence — concepts and terminology* (ISO/IEC JTC 1/SC 42; defines "AI agent" + 110 terms): <https://webstore.ansi.org/standards/iso/isoiec229892022>
- NIST — AI Agent Standards Initiative (agentic AI terminology/governance): <https://www.nist.gov/>

**Tier 2 — Linux Foundation, Agentic AI Foundation (AAIF) (neutral steward, announced 2025-12-09; governs the two standards GAL implements):**

- LF press release — formation of the AAIF (MCP + goose + AGENTS.md): <https://www.linuxfoundation.org/press/linux-foundation-announces-the-formation-of-the-agentic-ai-foundation>
- Anthropic — donating MCP to the AAIF: <https://www.anthropic.com/news/donating-the-model-context-protocol-and-establishing-of-the-agentic-ai-foundation>
- OpenAI — co-founding the AAIF (AGENTS.md): <https://openai.com/index/agentic-ai-foundation/>
- MCP spec — Architecture (host / client / server roles): <https://modelcontextprotocol.io/docs/concepts/architecture>
- AGENTS.md — open standard for AI coding agents: <https://agents.md>

**Tier 3 — Vendor first-party (product-level / illustrative):**

- Anthropic — Claude Code ("agentic coding tool"): <https://code.claude.com/docs/en/overview>
- OpenAI — Codex ("coding agent"): <https://openai.com/codex/>
- Google — Antigravity ("agentic development platform") + Gemini CLI→Antigravity CLI transition: <https://developers.googleblog.com/build-with-google-antigravity-our-new-agentic-development-platform/>, <https://developers.googleblog.com/an-important-update-transitioning-gemini-cli-to-antigravity-cli/>

**Tier 4 — Encyclopedic (neutral, tertiary):**

- Wikipedia — *Large language model* (model vs agent vs chatbot): <https://en.wikipedia.org/wiki/Large_language_model>
