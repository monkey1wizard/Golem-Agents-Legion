# Optional Capabilities

Writing checks use the same five-state preflight below. textlint may be
available through an explicit-profile local CLI or the official MCP route.
zhtw-mcp is optional context advice and is off for external verification by
default. Project terminology wins any conflict. See
[writing-quality.md](writing-quality.md) for the shared writing outcome
contract.

Portable rules for how ALL Golem agents check and route to optional external capabilities — Playwright MCP, OpenCLI, local-notes, and any future tool that extends GAL without becoming a core runtime dependency. This convention carries the binding behavior; human-facing setup/reference material lives in `docs/integrations.md`.

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

A configured capability is not necessarily exposed in the current session. Do not claim that an index or derived report is fresh or complete when its freshness or coverage is unknown. An empty result alone does not prove absence.

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
- local-notes (user-owned external note stores) follows this same five-state model: default off, machine-local enablement, `not-applicable` for most repos and tasks, and degrade to normal repo-local retrieval when the backend is not `ready`. Operational setup for local-notes lives in `docs/configuration.md`.

For writing checks, an unavailable or failed advisory route falls back to
self-review and one degradation disclosure until state changes. A repository
that marks a checker required in CI or an artifact gate fails closed when the
checker is unavailable, crashes, times out, or leaves blocking findings. GAL
does not require Node for ordinary runtime use, does not auto-install packages,
and does not create per-message auditor or checker-progress recursion.
