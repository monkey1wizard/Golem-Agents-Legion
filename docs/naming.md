# GAL Naming Authority

> **Single source of truth** for core term definitions and naming conventions in GAL.
> This file acts as the final decision authority for ambiguous names in code, comments, docs, or plans.
>
> Canonical language is `en` per `PROJECT_LANGUAGE`. This file receives no direct translations. It provides only a `zh-Hant` locale terminology presentation profile located at `docs/i18n/zh-Hant/terminology.zh-Hant.md`. Refer to [Semantic Authority vs. Locale Presentation Profiles](#semantic-authority-vs-locale-presentation-profiles).
> `docs/architecture.md` dictates **structure** and links here for **vocabulary**. This file avoids duplicating the crate DAG.

## Usage Guide

1. Find an ambiguous name here before introducing it. If it remains unregistered, register it per Naming Logic Rule 6 before usage.
2. Code comments and durable docs dictate **durable intent** instead of **provenance** per Rule 4. Plan-task IDs and migration history do not belong in shipped artifacts.
3. This file overrides any terminology disagreements between GAL components. Correct the divergent component to align with this document.

**Glossary conventions.** Term tables in Parts B–D implement the **arc42 Section 12 Glossary** structure for a single project-wide term registry. They incorporate the **Domain-Driven Design Ubiquitous Language** discipline. Every term includes a **Status** with `Validated` denoting the current canonical term and `Deprecated` denoting a retired term. Terms also include **Aliases** detailing accepted synonyms alongside incorrect names to avoid. The system emits retired terms into a [machine-readable block](#retired-terms-gate-input) consumed by the naming gate. Code identifiers, file paths, command names, and product names remain literal and untranslated. Core terms anchor to neutral authorities covering them before consulting vendor sources. These neutral authorities include standards bodies like ISO/IEC and NIST alongside the Linux Foundation's Agentic AI Foundation overseeing MCP and AGENTS.md. Refer to [authority tiers](#part-b--core-external-concepts-authority-anchored) and [Sources](#sources).

## Semantic Authority vs. Locale Presentation Profiles

This file acts as the **sole semantic authority** dictating what every core GAL term means. It covers canonical term meaning, canonical keys, identifiers, paths, commands, product names, and retired terms. No other document in any language may define, redefine, or compete with these established meanings. Term meanings reside exclusively in this file.

A **locale terminology profile** like `docs/i18n/zh-Hant/terminology.zh-Hant.md` serves a distinct and narrower function. Refer to `docs/devguide.md#documentation-conventions`. It records how settled terms from this file present to readers of a specific locale. It dictates presentation format in the local prose without altering or defining the term's underlying meaning. A profile's entries draw from a fixed set of presentation modes:

- **keep-en** — retain the English term. This serves as the default for product/core terms like `golem`, `GAL`, command names, and file paths. Naming Logic Rule 2/7 mandates literal retention for these.
- **localized** — a translated word or phrase native readers of the locale actively use.
- **bilingual-first-use** — the English term accompanied by a parenthetical local-language gloss upon its first mention in a document. Subsequent mentions use English only.
- **transliterated** — the term rendered in the locale's script utilizing sound rather than meaning.
- **contextual** — presentation depends on surrounding prose. The profile records a rule of thumb instead of a single fixed rendering.

A profile does not act as a translated copy of this file. It excludes Part A rules, Part B/C term tables, and the retired-terms list. It only maps settled English headwords onto a presentation mode. If a profile contradicts this file regarding term meaning, the profile is incorrect and requires correction. This file maintains ultimate authority on meaning per Rule 3.

---

## Part A — Naming Logic

These project-specific rules govern name creation and disambiguation.

1. **One term, one meaning.** A noun maps to exactly one concept across the project. If a concept requires a name and the obvious word is occupied, coin a qualified name. Avoid reusing the bare word.
2. **Reserved bare words, qualified everything else.** Two bare words remain *reserved* for one GAL meaning and operate unqualified:
   - **`agent`** → **golem agent** representing GAL's own specialist AI-agent personas. This reflects a deliberate product-design choice. GAL's agents are golem agents, thus plain "agent" refers to them exclusively.
   - **`model`** → **LLM** representing the brain. The terms "model" and "LLM" function as fixed synonyms in GAL.

   Every *other* concept vulnerable to English word overload like `agent`/`adapter`/`surface` requires qualification. An external AI coding tool acts as a **`coding agent`** and never a bare "agent". Other examples include **`MCP server`**, **`(file) projection backend`**, **`generated runtime adapter`**, and **`runtime surface`**. Bare usage of an unreserved overloaded word remains forbidden in code identifiers and durable docs.
3. **Entities vs roles.** The same external **coding agent** receives a name based on its axis role. It acts as a `runtime` when targeted for adapter generation. It acts as an `executor` when spawned for a dispatch stage. It acts as an `MCP host` when consuming MCP servers. Never utilize one role-name on an alternative axis. Never refer to a coding agent as a bare "agent" because that term designates a golem agent.
4. **Provenance lives in plan memory, not in durable artifacts.** Plan-task IDs including `T-NN`, `R-NN`, `TP-NN`, `FU-NN`, and `BUG-X` act as *provenance*. Migration narration phrasing like "repointed at", "strangler seam", or "until then" also acts as provenance. These elements reside strictly in `.dev/**`. Shipped code, comments, and durable docs capture *durable intent* exclusively.
5. **Names reflect the current owner, not migration history.** A module/file/symbol name must describe its present role or crate. Utilize `install_orchestration` instead of `legacy_plugins`.
6. **Register new overloaded-prone terms here before use.** Detail their axis and owner upon registration.
7. **Identifier formation (house style).** Follow these guidelines to construct a multi-word identifier:
   - **Precedence — common standard first, house style second.** The language's common standard provides authority and overrides house style upon conflict. For Rust, this entails the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/naming.html) and RFC 430 detailing casing, trait/constructor conventions, and acronyms. This GAL house style applies **only** where the standard permits a free choice regarding word selection, word order, or headword form. It **never** overrides the standard.
   - **Data** (types, fields, variables, modules) = **(verb→adjective) + Noun**. A verb transforms into a participle or gerund adjective modifying a head noun: `CodingAgent`, `DefinedName`, `ResolvedRoot`, `GeneratedAdapter`.
   - **Functions / methods** = **verb-led action phrase** where the verb retains its original form: `define_name`, `resolve_runtime`, `generate_adapter`. Avoid the adjective form like `defined_name()` because it names the result instead of the action.
   - **Canonical headword = `UpperCamelCase`, no underscores** (`CodingAgent`). This represents the form registered in this file and utilized as the type name. **Casing then defers to the language / item-kind standard.** For Rust under RFC 430, employ `snake_case` for fields, locals, modules, and functions (`coding_agent`). Employ `SCREAMING_SNAKE_CASE` for consts. This rule dictates word choice, word order, and headword form without governing casing. The language dictates casing.
8. **No generic bucket names.** Avoid naming a crate, module, or file utilizing a catch-all plural container word such as `adapters`, `services`, `providers`, `utils`, `helpers`, `common`, `managers`, `handlers`, or `misc`. These terms create junk drawers and introduce the overload forbidden by Rule 1. Name a unit by its **single responsibility**: utilize `projection` instead of `adapters`, `executors` instead of `dispatch::adapters`, and `mcp::serializers` instead of `providers`. If a generic word proves unavoidable, restrict it to **at most one** occurrence project-wide and register it here with its defined scope.
9. **No accidental name duplication, with standard slots as the only exception.** Target a single occurrence for a folder name, file name, or function name to ensure searches yield exactly one location. Avoid reusing one identifier across different locations for separate concepts. **Exception — names fixed by an external standard or a deliberate GAL convention intentionally repeat.** Each occurrence represents an identical structural slot instead of an overload:
   - Cargo layout repeating once **per crate**: `src/`, `src/lib.rs`, `src/main.rs`, `mod.rs`, `Cargo.toml`, `tests/`, `build.rs`.
   - GAL command standard repeating once **per command dir**: `SKILL.md`, `SKILL.template.md`.
   - Trait / constructor method names acting as protocol slots: `new`, `default`, `from_*`, `to_*`, `as_*`, `into_*`, `iter`, `fmt`, and methods implementing a shared trait.
   - Per-runtime adapter standard filenames acting as literal external tokens limited to one per repo: `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.github/copilot-instructions.md`.

   A repeat failing to qualify as a recognized standard slot violates Rule 1 and Rule 8 and requires renaming. Note that `README.md` does not qualify as a repeatable slot here. GAL policy dictates exactly **one** canonical `README.md`. Translations utilize `README.<lang>.md`.

> **Authority for casing:** Rust follows the [Rust API Guidelines — Naming](https://rust-lang.github.io/api-guidelines/naming.html) (C-CASE, built on RFC 430). Utilize `UpperCamelCase` for types, traits, and enum variants. Utilize `snake_case` for functions, methods, variables, modules, and crates. Utilize `SCREAMING_SNAKE_CASE` for consts and statics. Treat acronyms as a single word (`Uuid`, not `UUID`). Refer to `plugins/gal-core/conventions/rust.md`.

---

## Part B — Core external concepts (authority-anchored)

These definitions pin to authoritative sources and represent the vocabulary GAL adopts from the broader ecosystem. Do not redefine them locally.

**Authority tiers** (prefer the most neutral covering the term):

1. **Standards bodies** (neutral, formal) — ISO/IEC JTC 1/SC 42, NIST. This serves best for the genus *AI agent* and core AI terminology.
2. **Linux Foundation — Agentic AI Foundation (AAIF)** (neutral steward since 2025-12-09) — governs the two open standards GAL implements (MCP and AGENTS.md, alongside goose). This provides higher authority than any single vendor.
3. **Vendor first-party** — Anthropic / OpenAI / Google. This remains authoritative for their own product names. Treat category claims as marketing unless tier 1–2 echo them.
4. **Encyclopedic** — Wikipedia. This operates as neutral but tertiary.

**Row order is semantic, not alphabetical. Do not sort this table.** Rows descend from the genus (*AI agent*) through the categories GAL targets, then reach the standards GAL implements. The three MCP role rows stay in spec order — host, then client, then server — because the *Do not confuse with* column reads as a chain: the host coordinates clients, and each client holds one connection to one server. Alphabetizing would place *client* before *host* and break that chain. For alphabetical lookup, use the core table in `docs/i18n/zh-Hant/terminology.zh-Hant.md`, which covers every headword here and cites its Part.

| Term | Status | Aliases / a.k.a. | Canonical definition | Authority | Do **not** confuse with |
| --- | --- | --- | --- | --- | --- |
| **AI agent** (genus) | Validated | intelligent agent | An entity perceiving, deciding, and acting toward goals defined as formal AI terminology. | **ISO/IEC 22989:2022** (defining "AI agent" among 110+ terms) alongside the NIST AI Agent Standards Initiative. | a chatbot or an LLM. In GAL, bare **`agent`** equals golem agent per Part C. Both golem agents and coding agents represent *instances* of this genus. |
| **model** / **LLM** | Validated | LLM (fixed synonym) | A neural network trained on vast text for language tasks representing the brain. | Wikipedia covering *Large language model* alongside ISO/IEC 22989 covering model and AI system. | the agent or chatbot built on top of it. |
| **chatbot** / **AI assistant** | Validated | AI assistant | A consumer-facing conversational application built on an LLM including ChatGPT, Claude, and Gemini as standalone apps. | Wikipedia. | the LLM itself or a coding agent. |
| **coding agent** | Validated | agentic coding tool, agentic development platform | An AI agent reading a codebase, editing files, running commands, and integrating with dev tools. This forms the category GAL targets. | **Neutral**: Linux Foundation **AAIF** defines AGENTS.md as guidance for **AI coding agents**. **Vendor**: Anthropic defines Claude Code as an agentic coding tool. OpenAI defines Codex as a coding agent. Google defines Antigravity as an agentic development platform. | the model powering it or the surface hosting it. |
| **surface** / **interface** | Validated | interface | *Where* a coding agent runs including terminal/CLI, IDE, desktop app, or web. One agent can utilize several. | Anthropic defines Claude Code as available across terminal, IDE, desktop app, and browser. | the agent itself (CLI does not define the agent's category). |
| **MCP** | Validated | Model Context Protocol | Model Context Protocol representing the universal standard protocol for connecting AI models to tools, data, and applications. | **Linux Foundation AAIF** (steward since 2025-12-09 following Anthropic's donation). Spec available at modelcontextprotocol.io. | — |
| **MCP host** | Validated | (the agent's MCP role) | The AI application coordinating MCP clients. A coding agent **acts** as an MCP host. | MCP spec Architecture (identifying Claude Code as the host example). | MCP server. |
| **MCP client** | Validated | — | The component inside the host holding one connection to one MCP server. | MCP spec. | the host or the server. |
| **MCP server** | Validated | the provider | A program that **provides** tools, resources, or context to clients. The *server* acts as the provider. | MCP spec. | the host. |
| **AGENTS.md standard** | Validated | — | A simple, universal standard providing **AI coding agents** consistent project-specific guidance. GAL generates AGENTS.md alongside per-agent variants. | **Linux Foundation AAIF** (donated by OpenAI, 2025-12-09). | a GAL-specific invention. |

> **Retired term:** `MCP provider` (previously indicating a coding agent requiring MCP config) is **wrong** and removed. The agent acts as the **MCP host** and the **server** acts as the provider. The correct GAL concept remains *per-runtime MCP server configuration* per Part C.

---

## Part C — GAL-internal vocabulary (how external concepts map into GAL)

This details where GAL's identifiers attach to external concepts. Owner cells identify the **current** crate or module.

**Row order is semantic, not alphabetical. Do not sort this table.** Terms that encode a distinction sit next to each other so the distinction stays visible while reading. The opening `agent` / `runtime` / `executor` / `model` block is the worked example for Rule 2 and Rule 3, showing one coding agent carrying a different name on each axis. Alphabetizing separates those four rows and destroys the point they make. The remaining rows group by subject in this sequence — MCP config, projection, roots, plan memory, orchestration, retrieval principles, provenance, then `config.json` keys. For alphabetical lookup, use the core table in `docs/i18n/zh-Hant/terminology.zh-Hant.md`.

| GAL term | Status | Aliases | = which concept | Why this name / axis | Owner |
| --- | --- | --- | --- | --- | --- |
| **agent** (bare) = **golem agent** | Validated | golem agent (reserved bare `agent`) | a GAL specialist AI-agent persona (golem-architect, golem-tester) | **Reserved word.** Plain "agent" in GAL designates a golem agent via deliberate product-design. A golem agent qualifies as a genuine AI agent (ISO genus) representing a *role* performed by a coding agent. It does **not** signify a coding agent, runtime, or executor. | `plugins/gal-core/agents/` |
| **runtime** | Validated | (coding agent, generation axis) | a **coding agent** targeted by GAL | GAL's code handle for a coding agent. It remains `runtime` instead of bare `agent` because **`agent` is reserved for golem agents**. Axis: *target of adapter generation*. | `gal_foundation::runtime` (`VALID_RUNTIMES`) |
| **executor** | Validated | (coding agent, dispatch axis) | a **coding agent** invoked headlessly for a dispatch stage | Axis: *who runs this task*. This targets the same entity as a runtime fulfilling a different role. | `crates/dispatch` |
| **model** = **LLM** | Validated | LLM (reserved) | the LLM brain utilized by a coding agent | **Reserved word.** In GAL, "model" designates the LLM exclusively (e.g. `claude-opus-4-8`). Selected by executor-routing. | `config.json#executorRouting` |
| **per-runtime MCP server config** | Validated | — | the MCP **host** configuration generated per runtime | Replaces the inaccurate "MCP provider". GAL serializes each runtime's MCP host config where the agent acts as host consuming servers. Subset: 5 of 6 runtimes (excluding `gemini`). | `crates/mcp` (`mcp::serializers`) |
| **projection surface** (surface) | Validated | surface | where a runtime encounters GAL content on a machine | Axis: *where content lands* covering skill, command, or machine surface. | `crates/projection` |
| **projection** | Validated | — | the act of materializing canonical content onto a surface | Includes junction, symlink, copy, or atomic-swap. | `crates/projection` |
| **generated runtime adapter** | Validated | runtime adapter | the instruction file generated for runtime consumption | Acts as a product instead of a code module. See the member table for the specific file per runtime. | `crates/projection` + `gal-engine::render` |
| **(file) projection backend** | Validated | — | the code translating skills and commands into surface files | Axis: *code role*. This does not act as an "executor adapter". | `crates/projection` |
| **canonical root** | Validated | — | the rendered superset plugin root | Refers to `~/.gal/plugins/gal/` as the install product. | `gal-engine::render` |
| **source root** | Validated | — | the GAL source directory resolved by cwd-walk | Located automatically without requiring `galRoot` or `devMode` config. | `crates/cli::render` |
| **source plan / execution prompt / state** | Validated | — | `.dev/plans/<slug>.md` / `.dev/plans/<slug>.prompt.md` / `.dev/state.md` | Represents the three plan-memory layers per `plugins/gal-core/workflows/coding.md`. | repo |
| **dispatch / pipeline** | Validated | — | executor spawn (local + SSH remote lane) / local orchestration | Forms the orchestration DAG: `dispatch ← pipeline`. Remote (SSH) execution acts as a spawn lane composed within `dispatch::run` instead of a standalone crate per architecture § [SSH Spawn Lanes](architecture.md#ssh-spawn-lanes). | same-named crates |
| **local-first** | Validated | repo-first retrieval | the Core retrieval principle | Instructs agents to search the codebase, docs, and repo-owned state prior to querying optional external note backends. | `plugins/gal-core/skills/local-first-search`, `plugins/gal-core/workflows/research.md` |
| **local-notes** | Validated | external-note backend contract | the optional-capability lane targeting user-owned notes | Acts as an app-agnostic, default-off, machine-local note backend contract. It is never a Core prerequisite. | `plugins/gal-core/conventions/optional-capabilities.md` |
| **Personal Enhancement** | Validated | personal layer | machine-local opt-in behavior layered over Core | Encompasses owner-specific workflows, paths, or rituals remaining default-off and external to the Core contract. | `plugins/gal-core/conventions/core-vs-personal.md` |
| **plan-task ID** | Validated | T/R/TP/FU/BUG | `T/R/TP/FU/BUG` identifiers | *Provenance.* Permitted only in `.dev/**` per Rule 4. | `.dev` |
| **secrets** | Validated | — | `config.json#secrets` — credential key-value map | Employs UPPER_SNAKE key names with secret-bearing string values like API keys. Distinct from `vars`. Represents the exclusive secret-bearing section in `config.json`. | `crates/mcp::resolver`, `git_filters::run_clean_inner` |
| **executorRouting** | Validated | — | `config.json#executorRouting` — role routing grouped by consumer | Sole routing source. Groups roles **by consumer** into `pipeline` and `planning` alongside a shared `executors` default-model block. A `pipeline` entry can include `sshTarget`/`remoteWorkdir` to direct execution over the SSH lane per manual.md. | `crates/dispatch::routing` |
| **pipeline** (group) | Validated | — | `config.json#executorRouting.pipeline` — dispatch-role group | Closed allowlist covering `{CODER, TESTER, AUDITOR}` flattened into the dispatch table for phase offload and SSH lanes. Bypasses unknown roles with a warning. Distinct from a CI/CD pipeline. | `crates/dispatch::routing` |
| **planning** (group) | Validated | — | `config.json#executorRouting.planning` — planning-review-role group | Closed allowlist covering `{ARCHITECT, ANALYST, DESIGNER, RELEASER}` validated for diagnostics without entering the dispatch table. Read by `resolve_codex_routing` to define the Codex native-subagent model configuration. | `crates/projection::resolve_codex_routing` |
| **_galProjection** | Validated | — | top-level key in `plugins.lock.json` for the GAL projection registry | Written by `projection::persist`. Contains projection paths and source attribution. **Never modified by the resolver.** | `crates/projection::support` |

> **Settled — machine config is single-file:** `~/.gal/config/config.json` serves as the sole machine-config source carrying GAL-core settings, `secrets`, and `executorRouting`. No fallback exists. `plugins.lock.json` retains mixed-ownership where the resolver writes its namespace and `crates/projection` writes `_galProjection`.
>
> **Settled — the code handle stays `runtime`** instead of `coding_agent`. Bare `agent` remains **reserved for golem agent** by product design, prohibiting its use as an external handle. `runtime` avoids churn over the qualified `coding_agent`. The canonical external term stays **`coding agent`** and the internal handle stays **`runtime`**.

---

## Retired Terms (gate input)

This machine-readable list is consumed by the naming gate in `crates/cli/src/gal/naming_gate.rs`. Each non-comment line between the markers identifies a **retired phrase** forbidden in durable surfaces. Durable surfaces encompass everything outside provenance homes in `.dev/**`, the naming definition sites, and derived carriers regenerated from those sources.
Matching is case-insensitive and evaluates the whole phrase. Plan-task ID provenance is enforced by a separate regex in the gate.

The scanned set is **git-proven, never a filesystem walk**. A full-tree scan enumerates `git ls-files -z` and a staged scan enumerates `git diff --cached --diff-filter=ACMR -z`, reading each staged blob through `git show`. Untracked machine-local files are therefore out of scope by construction, and a nested checkout never leaks into the parent repo's scan. The gate **fails closed**: an inventory that cannot be obtained, a path that cannot be decoded, or tracked content that cannot be read or decoded aborts the scan with an error instead of reporting a clean result. Callers propagate that error — `gal naming-gate` exits non-zero, and the planning, refining, and finalize checks record a failing check with the error summary.

```text
# RETIRED-TERMS v1 — one retired phrase per line. Lines starting with `#` and blank lines are ignored.
# Format is stable: the gate reads only this fenced block, by these BEGIN/END comment fences.
# RETIRED-TERMS:BEGIN
MCP provider
executor-routing.json
ccync
# RETIRED-TERMS:END
```

> **Retired term:** `ccync` is forbidden outside this definition site. This document names the literal solely to enforce its exclusion elsewhere via the naming gate.

---

## Part D — Member registry (coding agents targeted by GAL)

The six `VALID_RUNTIMES` represent **coding agents**, not models. Canonical preference order matches `gal_foundation::runtime::VALID_RUNTIMES`.

| runtime key | Coding agent (product) | Vendor | Generated adapter file | per-runtime MCP config? |
| --- | --- | --- | --- | --- |
| `copilot` | GitHub Copilot | GitHub / Microsoft | `.github/copilot-instructions.md` | yes |
| `antigravity` (`agy`) | Google Antigravity | Google | `.agents/rules/gal.md` | yes |
| `gemini` | Gemini CLI | Google | `GEMINI.md` | **no** |
| `codex` | OpenAI Codex | OpenAI | `AGENTS.md` | yes |
| `opencode` | opencode | open source (SST) | `AGENTS.md` | yes |
| `claude` | Claude Code | Anthropic | `CLAUDE.md` | yes |

> **Watch item:** Google is transitioning **Gemini CLI → Antigravity CLI**. GAL lists both `gemini` and `antigravity` as runtimes. Evaluate a merge once the transition concludes.

---

## Sources

Authority anchors for Part B by tier.

**Tier 1 — Standards bodies (neutral, formal):**

- ISO/IEC 22989:2022 — *Artificial intelligence — concepts and terminology* (ISO/IEC JTC 1/SC 42 defining "AI agent" plus 110 terms): <https://webstore.ansi.org/standards/iso/isoiec229892022>
- NIST — AI Agent Standards Initiative (agentic AI terminology and governance): <https://www.nist.gov/>

**Tier 2 — Linux Foundation, Agentic AI Foundation (AAIF) (neutral steward announced 2025-12-09 governing the two GAL-implemented standards):**

- LF press release — formation of the AAIF: <https://www.linuxfoundation.org/press/linux-foundation-announces-the-formation-of-the-agentic-ai-foundation>
- Anthropic — donating MCP to the AAIF: <https://www.anthropic.com/news/donating-the-model-context-protocol-and-establishing-of-the-agentic-ai-foundation>
- OpenAI — co-founding the AAIF: <https://openai.com/index/agentic-ai-foundation/>
- MCP spec — Architecture covering host, client, and server roles: <https://modelcontextprotocol.io/docs/concepts/architecture>
- AGENTS.md — open standard for AI coding agents: <https://agents.md>

**Tier 3 — Vendor first-party (product-level and illustrative):**

- Anthropic — Claude Code: <https://code.claude.com/docs/en/overview>
- OpenAI — Codex: <https://openai.com/codex/>
- Google — Antigravity and the Gemini CLI transition: <https://developers.googleblog.com/build-with-google-antigravity-our-new-agentic-development-platform/>, <https://developers.googleblog.com/an-important-update-transitioning-gemini-cli-to-antigravity-cli/>

**Tier 4 — Encyclopedic (neutral, tertiary):**

- Wikipedia — *Large language model*: <https://en.wikipedia.org/wiki/Large_language_model>
