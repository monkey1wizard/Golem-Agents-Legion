# Naming Convention

The project-wide naming authority is **`docs/glossary.md`** (arc42 §12 Glossary + DDD Ubiquitous
Language). It is the single source of truth for what every core term means and how names are
coined. When a name in code, comments, docs, or plans is ambiguous, that file decides. This
convention inlines the rules every Golem agent must apply at cold-start; read the full authority
only when you need a term's definition, authority anchor, or the member registry.

## Rules every agent applies

1. **Reserved bare words** — bare **`agent`** always means a **golem agent** (GAL's own specialist
   personas); bare **`model`** always means the **LLM**. Never use either bare word for anything
   else.
2. **Qualify every other overloaded word.** An external AI coding tool is a **`coding agent`**
   (never bare "agent"); its GAL code handle is `runtime` (generation axis) or `executor` (dispatch
   axis). The agent is the **MCP host**; the **server** is the provider — there is no "MCP provider".
3. **Provenance stays in plan memory.** Plan-task IDs (`T-NN`, `R-NN`, `TP-NN`, `FU-NN`, `BUG-X`)
   and migration narration ("strangler", "until then", "repointed at…") are allowed **only** under
   `.dev/**`. Shipped code, comments, and durable docs state durable intent
   only. The naming gate enforces this.
4. **Names reflect the current owner, not history** (e.g. `install_orchestration`, not
   `legacy_plugins`).
5. **No generic bucket names** — no `adapters`, `services`, `providers`, `utils`, `helpers`,
   `common`, `managers`, `handlers`, `misc`. Name a unit by its single responsibility.
6. **Register a new overloaded-prone term in `docs/glossary.md` before using it.**
7. **Identifier formation** — common language standard first, GAL house style second (house style
   only fills the standard's free choices, never overrides it). Data names = `(verb→adjective) +
   Noun` (`CodingAgent`, `ResolvedRoot`); functions = verb-led (`resolve_runtime`); canonical
   headword is `UpperCamelCase`, no underscores. **Casing defers to the language standard** — for
   Rust, RFC 430 / the Rust API Guidelines (see [rust.md](rust.md)).
8. **No accidental duplicate folder/file/function names** except recognized standard slots (Cargo
   `src/`/`mod.rs`/`Cargo.toml`, GAL `SKILL.md`/`SKILL.template.md`, trait/constructor methods,
   per-runtime adapter filenames). Exactly one canonical `README.md`.

## Enforcement

The naming gate (`crates/gal-engine/src/naming_gate.rs`) scans durable surfaces for plan-task IDs
and retired terms (read from the `RETIRED-TERMS` block in `docs/glossary.md`). It excludes:
provenance homes (`.dev/**`); the naming **definition sites** (`docs/glossary.md`,
its translations, `conventions/naming.md`, `naming_gate.rs`); and **derived carriers** that are
regenerated from those sources and cannot be hand-fixed (`*.private.md`, baked
`commands/<name>/SKILL.md`, and the generated repo adapters `CLAUDE.md`/`AGENTS.md`). It runs in pre-commit (staged) and
`gal doctor` (full tree), and is **blocking**: `gal naming-gate` exits non-zero and the commit
is rejected on any hit. Do not introduce a name that the gate would flag.
