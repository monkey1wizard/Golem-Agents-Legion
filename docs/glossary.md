---
type: Reference
title: GAL Naming Authority
description: Single semantic authority for GAL terminology, covering naming rules, the canonical term registry, and retired terms.
tags:
  - terminology
  - naming
  - glossary
status: stable
---

# GAL Naming Authority

**Single source of truth** for core term definitions and naming conventions in GAL.
This document is the final authority for resolving ambiguous names in code, comments, documentation, and plans.

Per `PROJECT_LANGUAGE`, the canonical language is `en`. This document is not translated directly. Instead, locale terminology presentation profiles define how terms appear in [zh-Hant](i18n/zh-Hant/terminology.zh-Hant.md) and [ja](i18n/ja/terminology.ja.md). See [Semantic Authority vs. Locale Presentation Profiles](#semantic-authority-vs-locale-presentation-profiles).
`docs/architecture.md` defines **structure** and links here for **vocabulary** without duplicating crate relationships.

## Usage Guide

1. Look up any ambiguous name here before introducing it. If it is not yet registered, register it following Rule 6 before using it.
2. Write code comments and durable documentation to express **durable intent** rather than **provenance** (Rule 4). Keep plan-task IDs and migration history out of shipped artifacts.
3. This document overrides terminology conflicts across GAL components. Update divergent components to match this specification.

**Glossary conventions.** Term tables in Parts B–D follow the **arc42 Section 12 Glossary** structure for a project-wide registry, combined with the **Domain-Driven Design Ubiquitous Language** discipline.

- **Status**: Indicates whether a term is canonical (`Validated`) or retired (`Deprecated`).
- **Aliases / a.k.a.**: Lists accepted synonyms as well as incorrect terms to avoid.
- **Machine-readable retired terms**: Retired terms are collected in a [machine-readable block](#retired-terms-gate-input) checked by the automated naming gate.
- **Literals**: Code identifiers, file paths, command names, and product names remain literal and untranslated.
- **Grounding**: Core terms reference neutral standards bodies (such as ISO/IEC, NIST, and the Linux Foundation Agentic AI Foundation) before citing vendor sources. See [Authority Tiers](#part-b--core-external-concepts-authority-anchored) and [Sources](#sources).

## Semantic Authority vs. Locale Presentation Profiles

This document is the **sole semantic authority** for core GAL terminology. It defines canonical meanings, configuration keys, code identifiers, file paths, command names, product names, and retired terms. No document in any language may redefine or override these meanings.

A **locale terminology profile** (such as `docs/i18n/zh-Hant/terminology.zh-Hant.md` or `docs/i18n/ja/terminology.ja.md`) serves a distinct and narrower purpose (see [Multilingual Translation Architecture](architecture.md#multilingual-translation-architecture)). It specifies how canonical English terms are presented in local prose, without modifying their underlying definitions. Each entry in a profile uses one of five presentation modes:

- **keep-en** — Retain the original English term. This is the default for product names, core terms (`golem`, `GAL`), CLI commands, and file paths (Rules 2 and 7).
- **localized** — Use a standard translated word or phrase commonly accepted by native speakers in that locale.
- **bilingual-first-use** — On first mention in a document, display the local translation with the English term in parentheses. Subsequent mentions use either the local term or the English term consistently.
- **transliterated** — Phonetically transcribe the term into the locale script based on sound rather than meaning. (Currently unused in GAL).
- **contextual** — Presentation varies by context. The profile provides a heuristic rather than a single fixed rendering.

A profile is not a translation of this glossary. It omits the naming logic rules, complete definition tables, and retired-term gate blocks, mapping only canonical English headwords to their presentation mode. If a profile conflicts with this document regarding term meaning, this document governs and the profile must be corrected.

<a id="gal-glossary-terms"></a>
<!-- gal-terms:start -->
```json
{
  "schemaVersion": 1,
  "authority": "glossary",
  "sourcePolicy": "explicit-json-only",
  "completeRoster": true,
  "sourceTable": {"termColumns":["Term","GAL term","runtime key","Generated adapter file"],"aliasesColumns":["Aliases / a.k.a.","Aliases"],"definitionColumns":["Canonical definition","= which concept"],"headerOnlyConcepts":{"generated-adapter-file":"Generated adapter file","runtime-key":"runtime key","valid-runtimes":"runtime key","vendor":"Vendor"}},
  "locale": "en-US",
  "entries": [
    {"conceptId":"ai-agent","locale":"en-US","preferred":"AI agent","allowed":["AI agent","intelligent agent"],"forbidden":[],"mode":"localized","scope":["prose"],"exceptions":["definition table"],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"model","locale":"en-US","preferred":"LLM","allowed":["model","LLM"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":["definition table"],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"chatbot","locale":"en-US","preferred":"AI assistant","allowed":["chatbot","AI assistant"],"forbidden":[],"mode":"localized","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"coding-agent","locale":"en-US","preferred":"coding agent","allowed":["coding agent","agentic coding tool","agentic development platform"],"forbidden":["programming agent"],"mode":"contextual","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"surface","locale":"en-US","preferred":"interface","allowed":["surface","interface"],"forbidden":[],"mode":"localized","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"mcp","locale":"en-US","preferred":"MCP","allowed":["MCP","Model Context Protocol"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":["definition table"],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"mcp-host","locale":"en-US","preferred":"MCP host","allowed":["MCP host"],"forbidden":["MCP provider"],"mode":"keep-en","scope":["prose"],"exceptions":["retired terms block","definition table"],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"mcp-client","locale":"en-US","preferred":"MCP client","allowed":["MCP client"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"mcp-server","locale":"en-US","preferred":"MCP server","allowed":["MCP server"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"agents-md-standard","locale":"en-US","preferred":"AGENTS.md standard","allowed":["AGENTS.md standard"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"agent","locale":"en-US","preferred":"golem agent","allowed":["agent","golem agent"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"runtime","locale":"en-US","preferred":"runtime","allowed":["runtime"],"forbidden":["execution time"],"mode":"contextual","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"executor","locale":"en-US","preferred":"executor","allowed":["executor"],"forbidden":["executioner"],"mode":"contextual","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"gal-mcp-boundary","locale":"en-US","preferred":"GAL MCP boundary","allowed":["GAL MCP boundary"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"projection-surface","locale":"en-US","preferred":"projection surface","allowed":["projection surface"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"projection","locale":"en-US","preferred":"projection","allowed":["projection"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"generated-runtime-adapter","locale":"en-US","preferred":"generated runtime adapter","allowed":["generated runtime adapter","runtime adapter"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"projection-backend","locale":"en-US","preferred":"projection backend","allowed":["projection backend"],"forbidden":["mapping backend"],"mode":"contextual","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"canonical-root","locale":"en-US","preferred":"canonical root","allowed":["canonical root"],"forbidden":["standard root"],"mode":"contextual","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"source-root","locale":"en-US","preferred":"source root","allowed":["source root"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"source-plan","locale":"en-US","preferred":"source plan","allowed":["source plan"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"execution-prompt","locale":"en-US","preferred":"execution prompt","allowed":["execution prompt"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"state","locale":"en-US","preferred":"state","allowed":["state"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"dispatch","locale":"en-US","preferred":"dispatch","allowed":["dispatch"],"forbidden":["distribution"],"mode":"contextual","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"pipeline","locale":"en-US","preferred":"pipeline","allowed":["pipeline"],"forbidden":["waterfall"],"mode":"contextual","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"local-first","locale":"en-US","preferred":"local-first","allowed":["local-first"],"forbidden":["local priority"],"mode":"contextual","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"local-notes","locale":"en-US","preferred":"local-notes","allowed":["local-notes"],"forbidden":["local memos"],"mode":"contextual","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"personal-enhancement","locale":"en-US","preferred":"Personal Enhancement","allowed":["Personal Enhancement"],"forbidden":["personal improvement"],"mode":"contextual","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"plan-task-id","locale":"en-US","preferred":"plan-task ID","allowed":["plan-task ID"],"forbidden":["task identifier"],"mode":"contextual","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"secrets","locale":"en-US","preferred":"secrets","allowed":["secrets"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"executor-routing","locale":"en-US","preferred":"executorRouting","allowed":["executorRouting"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"pipeline-group","locale":"en-US","preferred":"pipeline","allowed":["pipeline"],"forbidden":[],"mode":"contextual","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"planning-group","locale":"en-US","preferred":"planning","allowed":["planning"],"forbidden":[],"mode":"contextual","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"gal-projection","locale":"en-US","preferred":"_galProjection","allowed":["_galProjection"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"generated-adapter-file","locale":"en-US","preferred":"generated adapter file","allowed":["generated adapter file"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"golem-agent","locale":"en-US","preferred":"golem agent","allowed":["golem agent"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"runtime-key","locale":"en-US","preferred":"runtime key","allowed":["runtime key"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"valid-runtimes","locale":"en-US","preferred":"VALID_RUNTIMES","allowed":["VALID_RUNTIMES"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"vendor","locale":"en-US","preferred":"vendor","allowed":["vendor"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"},
    {"conceptId":"agentic-coding-tool","locale":"en-US","preferred":"agentic coding tool","allowed":["agentic coding tool","agentic development platform"],"forbidden":[],"mode":"keep-en","scope":["prose"],"exceptions":[],"sourceAnchor":"docs/glossary.md#gal-glossary-terms"}
  ]
}
```
<!-- gal-terms:end -->

---

## Part A — Naming Logic

These project-specific rules govern name creation and disambiguation.

1. **One term, one meaning.** Each noun maps to exactly one concept across the project. If an obvious term is already in use, create a qualified name rather than reusing the bare noun.
2. **Reserved bare words, qualify everything else.** Only two bare words are reserved for specific GAL meanings without qualification:
   - **`agent`** → **golem agent**, referring exclusively to GAL specialist AI personas (such as `golem-architect` or `golem-tester`). Plain "agent" always means a golem agent.
   - **`model`** → **LLM**, the underlying foundation model powering an agent. "Model" and "LLM" are interchangeable synonyms in GAL.

   Every *other* overloaded term requires qualification. An external AI coding tool is a **`coding agent`**, never a bare "agent". Other examples include **`MCP server`**, **`(file) projection backend`**, **`generated runtime adapter`**, and **`projection surface`**. Using unreserved overloaded words without qualification is prohibited in code identifiers and durable documentation.
3. **Entities vs. roles.** An external **coding agent** is named by its role on a given axis:
   - A **`runtime`** when targeted for adapter generation.
   - An **`executor`** when spawned headlessly in a dispatch stage.
   - An **`MCP host`** when consuming MCP servers.

   Never mix role names across axes, and never call an external coding agent a bare "agent".
4. **Provenance belongs in plan memory, not durable artifacts.** Plan-task IDs (`T-NN`, `R-NN`, `TP-NN`, `FU-NN`, `BUG-X`) and migration phrasing ("repointed at", "strangler seam", "until then") represent *provenance*. Keep them strictly within `.dev/**`. Shipped code, comments, and durable documentation describe *durable intent* only.
5. **Names describe current responsibility, not migration history.** Module, file, and symbol names must reflect their current crate or role. For example, use `install_orchestration` rather than `legacy_plugins`.
6. **Register collision-prone terms before using them.** Record their concept, axis, and code owner in this document before introducing them.
7. **Identifier formation (house style).** Follow these guidelines to construct a multi-word identifier:
   - **Precedence — common standard first, house style second.** The language's common standard overrides house style on conflict. For Rust, this means the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/naming.html) and RFC 430 regarding casing, trait/constructor conventions, and acronyms. This GAL house style applies **only** where the standard permits a free choice regarding word selection, word order, or headword form. It **never** overrides the standard.
   - **Data** (types, fields, variables, modules) = **(verb→adjective) + Noun**. A verb transforms into a participle or gerund adjective modifying a head noun: `CodingAgent`, `DefinedName`, `ResolvedRoot`, `GeneratedAdapter`.
   - **Functions / methods** = **verb-led action phrase** where the verb retains its original form: `define_name`, `resolve_runtime`, `generate_adapter`. Avoid the adjective form like `defined_name()` because it names the result instead of the action.
   - **Canonical headword = `UpperCamelCase`, no underscores** (`CodingAgent`). This is the form registered in this file and used as the type name. **Casing then defers to the language or item-kind standard.** For Rust under RFC 430, use `snake_case` for fields, locals, modules, and functions (`coding_agent`). Use `SCREAMING_SNAKE_CASE` for consts. This rule dictates word choice, word order, and headword form without governing casing. The target programming language governs casing.
8. **No generic bucket names.** Do not name crates, modules, or files with catch-all terms such as `adapters`, `services`, `providers`, `utils`, `helpers`, `common`, `managers`, `handlers`, or `misc`. These words obscure responsibilities and cause naming collisions (Rule 1). Instead, name each unit after its **single responsibility**: use `projection` instead of `adapters`, and `executors` instead of `dispatch::adapters`. If a generic word is unavoidable, limit it to **at most one** occurrence across the project and register its scope in this file.
9. **No accidental name duplication, with standard slots as the only exception.** Target a single occurrence for a folder name, file name, or function name to ensure searches yield exactly one location. Avoid reusing one identifier across different locations for separate concepts. **Exception — names fixed by an external standard or a deliberate GAL convention intentionally repeat.** Each occurrence represents an identical structural slot instead of an overload:
   - Cargo layout repeating once **per crate**: `src/`, `src/lib.rs`, `src/main.rs`, `mod.rs`, `Cargo.toml`, `tests/`, `build.rs`.
   - GAL command standard repeating once **per command dir**: `SKILL.md`, `SKILL.template.md`.
   - Trait / constructor method names acting as protocol slots: `new`, `default`, `from_*`, `to_*`, `as_*`, `into_*`, `iter`, `fmt`, and methods implementing a shared trait.
   - Per-runtime adapter standard filenames acting as literal external tokens limited to one per repo: `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.github/copilot-instructions.md`.

   Any repeated name that is not a standard slot violates Rules 1 and 8 and must be renamed. Note that `README.md` is not a repeatable slot here. GAL policy permits exactly **one** canonical `README.md`. Translations use `README.<lang>.md`.

> **Authority for casing:** Rust follows the [Rust API Guidelines — Naming](https://rust-lang.github.io/api-guidelines/naming.html) (C-CASE, based on RFC 430). Use `UpperCamelCase` for types, traits, and enum variants. Use `snake_case` for functions, methods, variables, modules, and crates. Use `SCREAMING_SNAKE_CASE` for consts and statics. Treat acronyms as single words (`Uuid`, not `UUID`). See `plugins/gal-core/conventions/rust.md`.

---

## Part B — Core external concepts (authority-anchored)

These definitions pin to authoritative sources and represent the vocabulary GAL adopts from the broader ecosystem. Do not redefine them locally.

**Authority tiers** (prefer the most neutral source covering the term):

1. **Standards bodies** (neutral, formal) — ISO/IEC JTC 1/SC 42, NIST. This applies to the genus *AI agent* and core AI terminology.
2. **Linux Foundation — Agentic AI Foundation (AAIF)** (neutral steward since 2025-12-09) — governs the two open standards GAL implements (MCP and AGENTS.md, alongside goose). This provides higher authority than any single vendor.
3. **Vendor first-party** — Anthropic / OpenAI / Google. Authoritative for their own product names. Treat category claims as vendor marketing unless tier 1–2 echo them.
4. **Encyclopedic** — Wikipedia. Neutral but tertiary.

**Row order is semantic, not alphabetical. Do not sort this table.** Rows descend from the genus (*AI agent*) through the categories GAL targets, then reach the standards GAL implements. The three MCP role rows stay in spec order — host, then client, then server — because the *Do not confuse with* column reads as a chain: the host coordinates clients, and each client holds one connection to one server. Alphabetizing would place *client* before *host* and break that chain. For alphabetical lookup, use the core table in the [zh-Hant](i18n/zh-Hant/terminology.zh-Hant.md#gal-核心術語呈現表) or [ja](i18n/ja/terminology.ja.md#gal-中核用語表示表) locale profile. Each profile covers every headword here and cites its Part.

| Term | Status | Aliases / a.k.a. | Canonical definition | Authority | Do **not** confuse with |
| --- | --- | --- | --- | --- | --- |
| **AI agent** (genus) | Validated | intelligent agent | An autonomous entity that perceives its environment, makes decisions, and takes actions to achieve defined goals. | **ISO/IEC 22989:2022** (defining "AI agent" among 110+ terms) alongside the NIST AI Agent Standards Initiative. | A chatbot or an LLM. In GAL, bare **`agent`** equals golem agent per Part C. Both golem agents and coding agents represent *instances* of this genus. |
| **model** / **LLM** | Validated | LLM (fixed synonym) | A large neural network trained on vast text corpora for language understanding and generation (the reasoning engine). | Wikipedia covering *Large language model* alongside ISO/IEC 22989 covering model and AI system. | The agent or chatbot built on top of it. |
| **chatbot** / **AI assistant** | Validated | AI assistant | A consumer-facing conversational application built on an LLM, including ChatGPT, Claude, and Gemini as standalone apps. | Wikipedia. | The LLM itself or a coding agent. |
| **coding agent** | Validated | agentic coding tool, agentic development platform | An AI agent capable of reading code, editing files, running commands, and integrating with developer tools. This is the primary agent category GAL targets. | **Neutral**: Linux Foundation **AAIF** defines AGENTS.md as guidance for **AI coding agents**. **Vendor**: Anthropic defines Claude Code as an agentic coding tool. OpenAI defines Codex as a coding agent. Google defines Antigravity as an agentic development platform. | The model powering it or the surface hosting it. |
| **surface** / **interface** | Validated | interface | The environment where a coding agent runs (such as terminal/CLI, IDE, desktop app, or web interface). A single agent may support several surfaces. | Anthropic defines Claude Code as available across terminal, IDE, desktop app, and browser. | The agent itself (a CLI interface does not define the agent's category). |
| **MCP** | Validated | Model Context Protocol | Model Context Protocol. An open standard protocol for connecting AI models to tools, data sources, and applications. | **Linux Foundation AAIF** (steward since 2025-12-09 following Anthropic's donation). Spec available at modelcontextprotocol.io. | — |
| **MCP host** | Validated | (the agent's MCP role) | The AI application coordinating MCP clients. A coding agent **acts** as an MCP host. | MCP spec Architecture (identifying Claude Code as the host example). | MCP server. |
| **MCP client** | Validated | — | The client component within a host that maintains a dedicated connection to a single MCP server. | MCP spec. | The host or the server. |
| **MCP server** | Validated | the provider | A program that provides tools, resources, or prompt templates to MCP clients. | MCP spec. | The host. |
| **AGENTS.md standard** | Validated | — | An open standard that provides AI coding agents with consistent, project-specific guidance and instructions. GAL generates AGENTS.md alongside per-agent variants. | **Linux Foundation AAIF** (donated by OpenAI, 2025-12-09). | A GAL-specific invention. |

> **Retired term:** `MCP provider` (previously indicating a coding agent requiring MCP config) is **wrong** and removed. The agent acts as the **MCP host** and the **server** acts as the provider. A coding agent is therefore never something GAL configures as a provider — see *GAL MCP boundary* in Part C for what GAL does own.

---

## Part C — GAL-internal vocabulary (how external concepts map into GAL)

This details where GAL's identifiers attach to external concepts. Owner cells identify the **current** crate or module.

**Row order is semantic, not alphabetical. Do not sort this table.** Terms that encode a distinction sit next to each other so the distinction stays visible while reading. The opening `agent` / `runtime` / `executor` / `model` block is the worked example for Rule 2 and Rule 3, showing one coding agent carrying a different name on each axis. Alphabetizing separates those four rows and destroys the point they make. The remaining rows group by subject in this sequence — MCP config, projection, roots, plan memory, orchestration, retrieval principles, provenance, then `config.json` keys. For alphabetical lookup, use the core table in the [zh-Hant](i18n/zh-Hant/terminology.zh-Hant.md#gal-核心術語呈現表) or [ja](i18n/ja/terminology.ja.md#gal-中核用語表示表) locale profile.

| GAL term | Status | Aliases | = which concept | Why this name / axis | Owner |
| --- | --- | --- | --- | --- | --- |
| **agent** (bare) = **golem agent** | Validated | golem agent (reserved bare `agent`) | A specialist GAL AI persona (such as `golem-architect` or `golem-tester`) | **Reserved word.** Plain "agent" in GAL designates a golem agent by deliberate product design. A golem agent is a specialized role performed by a coding agent. It does **not** signify a coding agent, runtime, or executor. | `plugins/gal-core/agents/` |
| **runtime** | Validated | (coding agent, generation axis) | A **coding agent** targeted by GAL | GAL's code handle for an external coding agent. Named `runtime` rather than `agent` because **`agent` is reserved for golem agents**. Axis: *target of adapter generation*. | `gal_foundation::runtime` (`VALID_RUNTIMES`) |
| **executor** | Validated | (coding agent, dispatch axis) | A **coding agent** invoked headlessly for a dispatch stage | Axis: *who runs this task*. Targets the same entity as a runtime, operating on a different axis. | `crates/dispatch` |
| **model** = **LLM** | Validated | LLM (reserved) | The underlying LLM powering a coding agent | **Reserved word.** In GAL, "model" designates the LLM exclusively (e.g. `claude-opus-4-8`), selected via executor routing. | `config.json#executorRouting` |
| **GAL MCP boundary** | Validated | — | The separation between GAL's internal MCP manifest and a host's active MCP configuration | GAL declares and consumes its own MCP servers, materialized as the canonical `.mcp.json` in the plugin root. GAL manages **no** host's live MCP config — it never writes `claude_desktop_config.json`, `~/.copilot/mcp-config.json`, `~/.codex/config.toml`, `opencode.json`, or `mcp_config.json`. Each coding agent is an **MCP host** in its own right, not a GAL configuration target. | `plugins/gal-core/mcp.json` + `gal-engine::render` |
| **projection surface** (surface) | Validated | surface | Where a runtime encounters GAL content on a machine | Axis: *where content lands*, including skill directories, command paths, and machine surfaces. | `crates/projection` |
| **projection** | Validated | — | The operation of materializing canonical content onto a target surface | Implemented via junction, symlink, copy, or atomic swap. | `crates/projection` |
| **generated runtime adapter** | Validated | runtime adapter | The instruction file generated for runtime consumption | Handled as a generated product rather than a code module. See the member table in Part D for the specific file per runtime. | `crates/projection` + `gal-engine::render` |
| **(file) projection backend** | Validated | — | The engine code translating skills and commands into surface files | Axis: *code role*. Distinct from an executor adapter. | `crates/projection` |
| **canonical root** | Validated | — | The rendered superset plugin root | Refers to `~/.gal/plugins/gal/` as the primary install product. | `gal-engine::render` |
| **source root** | Validated | — | The GAL source directory resolved by directory traversal | Located automatically without requiring `galRoot` or `devMode` configuration. | `crates/cli::render` |
| **source plan / execution prompt / state** | Validated | — | `.dev/plans/<slug>.md` / `.dev/plans/<slug>.prompt.md` / `.dev/state.md` | Represents the three plan-memory layers per `plugins/gal-core/workflows/coding.md`. | repo |
| **dispatch / pipeline** | Validated | — | Executor dispatch (local and SSH remote lane) / local orchestration | Forms the orchestration DAG: `dispatch ← pipeline`. Remote SSH execution runs as a spawn lane composed within `dispatch::run` rather than as a standalone crate. See [Remote Dispatch (SSH Lane)](workflows.md#remote-dispatch-ssh-lane). | same-named crates |
| **local-first** | Validated | repo-first retrieval | The core retrieval principle | Directs agents to search the codebase, documentation, and repo-owned state prior to querying optional external note backends. | `plugins/gal-core/skills/local-first-search`, `plugins/gal-core/workflows/research.md` |
| **local-notes** | Validated | external-note backend contract | The optional-capability lane targeting user-owned notes | An app-agnostic, default-off, machine-local note backend contract. It is never a Core prerequisite. | `plugins/gal-core/conventions/optional-capabilities.md` |
| **Personal Enhancement** | Validated | personal layer | Machine-local opt-in behavior layered over Core | Encompasses user-specific workflows, paths, or rituals that remain disabled by default and external to the Core contract. | `plugins/gal-core/conventions/core-vs-personal.md` |
| **plan-task ID** | Validated | T/R/TP/FU/BUG | `T/R/TP/FU/BUG` identifiers | *Provenance.* Permitted only in `.dev/**` per Rule 4. | `.dev` |
| **secrets** | Validated | — | `config.json#secrets` — credential key-value map | Key-value map of credentials (such as API keys) using UPPER_SNAKE_CASE keys. This is the exclusive secret-bearing section in `config.json`, distinct from non-sensitive `vars`. | `crates/mcp::resolver`, `git_filters::run_clean_inner` |
| **executorRouting** | Validated | — | `config.json#executorRouting` — role routing grouped by consumer | Sole routing source. Groups roles **by consumer** into `pipeline` and `planning` alongside a shared `executors` default-model block. A `pipeline` entry can include `sshTarget`/`remoteWorkdir` to direct execution over the SSH lane documented in `docs/workflows.md`. | `crates/dispatch::routing` |
| **pipeline** (group) | Validated | — | `config.json#executorRouting.pipeline` — dispatch-role group | Closed allowlist covering `{CODER, TESTER, AUDITOR}` mapped into the dispatch table for phase offload and SSH lanes. Bypasses unknown roles with a warning. Distinct from a CI/CD pipeline. | `crates/dispatch::routing` |
| **planning** (group) | Validated | — | `config.json#executorRouting.planning` — planning-review-role group | Closed allowlist covering `{ARCHITECT, ANALYST, DESIGNER, RELEASER}` validated for diagnostics without entering the dispatch table. Read by `resolve_codex_routing` to define the Codex native-subagent model configuration. | `crates/projection::resolve_codex_routing` |
| **_galProjection** | Validated | — | Top-level key in `plugins.lock.json` for the GAL projection registry | Written by `projection::persist`. Contains projection paths, source attribution, and `pluginOwned` category mapping. **Never modified by the resolver.** | `crates/projection::support` |

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

The five `VALID_RUNTIMES` represent **coding agents**, not models. Canonical preference order matches `gal_foundation::runtime::VALID_RUNTIMES`.

| runtime key | Coding agent (product) | Vendor | Generated adapter file |
| --- | --- | --- | --- |
| `copilot` | GitHub Copilot | GitHub / Microsoft | `.github/copilot-instructions.md` |
| `antigravity` (`agy`) | Google Antigravity | Google | `.agents/rules/gal.md` |
| `codex` | OpenAI Codex | OpenAI | `AGENTS.md` |
| `opencode` | opencode | open source (SST) | `AGENTS.md` |
| `claude` | Claude Code | Anthropic | `AGENTS.md` |

> **No MCP column.** Each of these agents is an MCP host in its own right, but none of them receives an MCP config from GAL. See *GAL MCP boundary* in Part C.
>
> **Antigravity command surface:** GAL projects each command as an Agent Skill at `~/.gemini/antigravity-cli/skills/<name>/SKILL.md`. The repo-local `GEMINI.md` remains an Antigravity context bridge, not a selectable runtime key.

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
