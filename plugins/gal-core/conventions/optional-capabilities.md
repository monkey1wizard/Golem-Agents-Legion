# Optional Capabilities

Portable rules for how ALL Golem agents check and route to optional external capabilities — graphify, codebase-memory-mcp, Playwright MCP, OpenCLI, local-notes, and any future tool that extends GAL without becoming a core runtime dependency. This convention carries the binding behavior; human-facing setup/reference material lives in `docs/integrations.md`.

## Five-State Preflight

Every workflow answers the same questions in the same order before it uses any optional capability:

```text
applicability -> availability -> initialization status -> readiness -> route -> degrade
```

| State | Meaning | Owned by | Expected behavior |
| --- | --- | --- | --- |
| `not-applicable` | The current lane or task does not need this capability. | workflow layer | Skip the capability without surfacing setup work. |
| `unavailable` | The machine or runtime cannot access the capability. | machine-local install and runtime wiring | Use the documented fallback path. Do not pretend the capability ran. |
| `available-but-needs-init` | The capability exists, but first-time setup for this machine, repo, or runtime is incomplete. | capability-specific init contract | Do not auto-initialize during normal planning, review, or research. |
| `available-but-not-ready` | The capability is installed and initialized, but the current repo or task lacks the artifacts needed for this lane. | repo or task preconditions | Degrade to the documented non-tool path. |
| `ready` | The capability is applicable and all required preconditions are satisfied. | workflow layer after preflight | Route into the capability. |

### Decision Boundaries

- Applicability is a workflow concern.
- Availability is a machine-local concern.
- Initialization status is a capability-specific concern.
- Readiness is a repo or task concern.
- Routing and degradation are workflow concerns.

Personalization may affect availability and machine-local preferences. It must not redefine workflow semantics or readiness rules.

### Required Behavior

- Do not auto-install optional capabilities.
- Do not auto-run first-time initialization during normal planning, review, or research.
- Do not hide missing capabilities behind vague success language.
- Always define an explicit degrade path for each capability-enabled lane.
- Keep the normal GAL write-back targets unchanged unless a capability contract says otherwise.

## Structural-Retrieval Routing

structural-retrieval is the capability lane for locating structural targets faster and more accurately without turning any retrieval aid into a required dependency. It covers two aids:

- **graphify** — static structural context from a generated report; not an MCP lane.
- **codebase-memory-mcp** — live structural and symbol lookup through MCP; for doc-sync, native `git diff` remains the mandatory baseline.

This capability is always bounded the same way: advisory-only, degrade silently when a tool is not ready, never required for normal GAL planning, review, or doc-sync execution.

| Agent goal / lane | Preferred aid | What it returns | Fallback |
| --- | --- | --- | --- |
| planning: judge scope, module boundaries, or surprising cross-area connections before drafting or refining a plan | graphify | advisory report-level structure such as communities, god nodes, and notable cross-module links | continue with native codebase reading and normal planning flow |
| architect: reason about abstractions, coupling, ownership boundaries, or structural blast radius | graphify | advisory structural context that helps challenge a plan or branch against the repo's coarse architecture | continue with direct file reads, call-path inspection, and normal architecture review |
| auditor: check whether a change appears to create unexpected cross-community or cross-boundary effects | graphify | advisory cross-community context for whole-change review; not a replacement for diff-based audit | continue with standard diff review and direct code inspection |
| doc-sync: tighten affected-doc targeting after native file detection, especially when `codeRefs` include symbols | codebase-memory-mcp | advisory symbol-aware lookup and structural hints after readiness is confirmed; may refine doc-section targeting after `git diff` | keep the native `git diff` plus direct file-read baseline only |
| complement: use both broad structure and exact symbol targeting in the same task | start with graphify, then refine with codebase-memory-mcp when the second lane is ready | graphify gives coarse structure first; codebase-memory-mcp then narrows to exact files, symbols, or impacted sections | keep the coarse result only when available, otherwise fall back fully to native reading |

The two aids complement each other rather than compete: graphify is the better first pass for coarse structural orientation, codebase-memory-mcp is the better second pass for precise symbol-aware targeting. If either aid is absent, GAL continues without surfacing setup work as part of normal execution.

### Codex Capability Gating (configured ≠ exposed)

In Codex, the codebase-memory MCP lane is **capability-gated on the tools actually exposed in the session, not on config presence**. A `[mcp_servers.*]` entry in `~/.codex/config.toml` (which `gal doctor` may report as an advisory) means the server is *configured* — it does **not** guarantee that the codebase-memory graph tools (`search_graph`, `trace_path`, `get_code_snippet`, …) are actually exposed to the model in this session.

Therefore, before routing into the codebase-memory-mcp lane in Codex:

- Resolve tool availability through the five-state preflight above against the **exposed** tool set, never against the configured-server list.
- If the graph tools are **not exposed** in the session, treat the lane as `unavailable` and **fall back to repo-native discovery** (`rg` / direct file reads / `git diff`) — the same non-tool baseline the routing table already prescribes.
- Never block on the missing MCP tools, and never claim the graph tools were used when they were not exposed. A configured-but-unexposed server is a silent-degrade case, not an error and not a success.

This gate applies to any runtime where configured MCP servers can diverge from the tools exposed to the model; it is called out for Codex because that divergence is common there.

## Skill Surface Map

Three distinct skill surfaces exist, each with different ownership and projection behavior:

| Surface | Path | Shared? | Created by `gal refresh`? | Notes |
| --- | --- | --- | --- | --- |
| Canonical shared | `~/.agents/skills/` | Yes (Codex, OpenCode, Copilot) | Yes | The single source of truth for skills shared across multiple runtimes. Materialized directories with per-file byte comparison. |
| Claude-only | `~/.claude/skills/gal` | No (Claude exclusive) | No | Claude-specific surface. Must be created manually (junction/symlink to canonical root). `gal doctor` reports missing surface as warning with manual creation instructions. |
| OpenCode private | `~/.config/opencode/skills/` | No | No | Does not exist. OpenCode reads from the canonical shared surface. Any orphan directory at this path is pruned by `legacy_paths` when lockfile-attributed. |

The canonical shared surface (`~/.agents/skills/`) is the only skill projection that `gal refresh` writes for multiple runtimes. Claude's surface is separate and not auto-created. OpenCode has no private skill projection — it reads directly from the shared surface.

## Honest Degradation

- A missing or not-ready optional capability is never a workflow error and never a reason to fabricate success language.
- A downstream repo with no optional capability configured must proceed through native paths with no setup prompts and no false success.
- local-notes (user-owned external note stores) follows this same five-state model: default off, machine-local enablement, `not-applicable` for most repos and tasks, and degrade to normal repo-local retrieval when the backend is not `ready`. Operational setup for local-notes lives in `docs/manual.md`.
