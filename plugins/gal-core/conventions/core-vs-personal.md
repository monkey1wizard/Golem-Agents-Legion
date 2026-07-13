# Core vs Personal Convention

Define the first-class boundary between **GAL Core** and **Personal Enhancement**.

This convention applies to commands, skills, workflows, templates, docs, prompts, adapters, and
runtime defaults.

## Boundary Rule

- **GAL Core** is the portable contract shipped to every downstream repo.
- **Personal Enhancement** is any optional behavior layered on top of Core for one user's local
  setup, habits, or private assets.

Core must remain fully usable when every Personal Enhancement is disabled.

## Applicability Test

A behavior belongs in **Core** only when all of the following are true:

1. A downstream repo can use it after `gal init` without owner-specific paths, vaults, tools,
   rituals, or private data.
2. The default behavior is still correct when no machine-local note system, diary, or personal
   assistant tooling exists.
3. The contract describes durable shared behavior, not one user's personal workflow preference.

If any answer is no, the behavior is **Personal Enhancement** and must not be a Core default.

## Personal Enhancement Rules

- Personal Enhancement is always **opt-in**.
- Personal Enhancement is always controlled by **machine-local configuration**.
- Personal Enhancement defaults to **disabled**.
- Personal Enhancement may extend Core, but must not redefine Core prerequisites.
- Personal Enhancement must not make private paths, private vault structure, or private rituals a
  required dependency for shipped behavior.

## Ownership Rules

- Core may define generic principles such as `local-first` retrieval order or portable collaborative
  tool contracts.
- Core may reference optional app-specific or tool-specific integrations only as non-required,
  default-off capability lanes.
- Core must not ship owner-personalized routing such as PARA assumptions, Guide resolution,
  semantic-note assistants, private diary locations, or personal vault conventions.
- When a feature has both a portable layer and a personal layer, split them: keep the portable
  contract in Core and move the personal behavior to machine-local opt-in configuration.

## Decision Guidance

- Prefer the smallest portable Core that solves the shared problem.
- Do not build a generic plugin framework just to host one personal workflow.
- When in doubt, classify the behavior as Personal Enhancement until a portable Core contract is
  proven.

## Personal Content Home (P2)

Machine-local personal content lives in `~/.gal/local/` (the **P2 personal content home**):

### User-owned content (GAL read-only — never written or deleted by GAL)

- `~/.gal/local/skills/<name>/SKILL.md` — hand-placed personal skill files
- `~/.gal/local/mcp.json` — personal MCP server manifest (same `"servers"` key format as core)
- `~/.gal/local/conventions/<lang>.md` — personal coding-style convention files (see Language Convention Boundary below)

**Read-only boundary:** GAL reads but never writes `local/skills/`, `local/mcp.json`, or `local/conventions/` (all user-authored content).

**Projection is presence-based:** placing a file at the personal root is itself the opt-in — there is no config flag to enable. Absence of the directory keeps every render byte-identical to a Core-only render.

**Projection via canonical-root render:**

- `render_canonical_root` merges personal skills into `ScannedComponents` (core first, personal appended; core-wins on collision) before `render_to_temp` when the personal skills directory exists, and merges personal MCP servers into the canonical `.mcp.json` (core-wins on collision) when the personal MCP manifest exists.
- The canonical root is the single render target for Claude; personal content reaches Claude through this path.
- Absent personal directory/file → render output is byte-identical to a Core-only render.
- Cross-machine sync of `~/.gal/local/` is not a Core responsibility; it is left to the user or a future Personal Enhancement.

Third-party plugin/skill catalogs and cross-agent installation — git-cloned plugin management, a plugin lockfile, and `gal plugin`-style commands — are **not** part of GAL. That is the separate **ccync** product.

## Language Convention Boundary

Owner-personal coding house style (e.g. a specific C#/.NET, Go, or TypeScript style guide) is **Personal Enhancement**, never a GAL Core convention. Core's own `plugins/gal-core/conventions/` ships only neutral, downstream-agnostic conventions (currently just `rust.md`, GAL's own dev standard for its `crates/` workspace — not a rule injected into other repos as house style).

Language conventions reach a downstream repo's adapters through **three sources**, all reference-only or user-authored — GAL never bundles a language-specific style as Core:

1. **Personal convention files** (`~/.gal/local/conventions/<lang>.md`) — user-written, per-repo always-on. The repo-adapter render selects a file when its stem (or a recognized alias) matches the project's Language line; a generic/non-language stem always matches. This is the direct successor to the old bundled `csharp.md`/`go.md`/`typescript.md` — the same always-on behavior, moved to the correct machine-local layer. A labeled example ships at `plugins/gal-core/templates/csharp-convention.example.md` (inert; never selected or swept into any corpus).
2. **Installed agent-plugin detection** (Detected Language Skills) — read-only. GAL scans installed skill roots (Claude plugin cache, shared agents skills, Copilot skills, agy skills) for skills whose name or origin path matches the Language line, and renders a **reference-only** block (name + origin + a standing load-first instruction). GAL never copies, vendors, manages, or deletes a third-party skill's content — detection is a citation, not a dependency.
3. **Personal skills** (`~/.gal/local/skills/`) — on-demand, loaded by the agent when named, same as any other personal skill (see Personal Content Home above).

**Public official plugin vs. self-written personal file:** an officially-installed plugin (source 2) has its own lifecycle — install, update, uninstall — owned by the hosting agent (Claude Code, Codex, etc.) today, and by the future **ccync** product for cross-agent management. GAL is a **consumer** of that lifecycle (detect + reference), never a manager of it. A self-written personal convention file (source 1) has no such lifecycle — the user owns its content and its presence directly; GAL only reads it.

A repo-level "Personal Conventions = off" row in `.dev/project.md`'s Tech Stack table disables sources 1 and 2 for that repo (recommended for public repos, so their tracked adapters never embed owner-machine content); source 3 (on-demand skills) is unaffected since it never enters an always-on adapter.

## Examples

- `local-first` retrieval order for software-engineering work is Core.
- A `local-notes` collaborative-tool contract is Core when it stays optional and default-off.
- A private Obsidian vault path, diary ritual, PARA layout, or personal semantic-search helper is
  Personal Enhancement.
- `~/.gal/local/` personal content projection (P2 personal content home) is a machine-local
  Personal Enhancement, opted into by placing a file (presence-based, no config flag); Core works
  fully without it.
- Third-party plugin/skill catalogs are handled by the separate ccync product, not GAL.
- An owner's personal C#/Go/TypeScript house style is Personal Enhancement (`~/.gal/local/conventions/`), never a GAL Core convention.
