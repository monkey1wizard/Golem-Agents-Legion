# Installation Topology

This document explains how GAL is installed into tool environments and how the same methodology stays synchronized across machines.

## Runtime Layers

GAL uses four practical runtime layers.

| Layer | Location | Purpose |
| --- | --- | --- |
| Layer 1 | `~/golem-agents-legion/` | Canonical methodology source |
| Layer 1.5 | `~/.copilot/` and `~/.gemini/` | Tool-facing symlinks and baked command skills |
| Layer 2 | `<target-repo>/.dev/` | Per-repo working context and state |
| Layer 3 | Generated adapter files in target repos | `AGENTS.md` (shared cross-CLI) + tool-specific shims |

README keeps the high-level three-layer model. This document expands the runtime installation details that sit between the repo and the tools.

## Cross-CLI Support Matrix

This matrix is the canonical reference for how each CLI runtime integrates with GAL. Use it when adding a new CLI or auditing an existing one.

### Machine-Layer Install

| CLI | Machine Install | Notes |
| --- | --- | --- |
| Copilot CLI | `~/.copilot/agents/` + `~/.copilot/skills/` | Symlinks + baked `SKILL.md` via Setup-Machine |
| Gemini CLI | `~/.gemini/commands/` + `~/.agents/skills/` + `settings.json` bridge | Native GAL slash commands are generated into `.gemini/commands`; reusable non-command skills are shared from `.agents/skills`; bridge writes `AGENTS.md` and `GEMINI.md` to `context.fileName`; Setup-Machine also merges `mcpServers` |
| Codex CLI | `~/.codex/skills/` + `~/.agents/skills/` | GAL command skills live in `.codex/skills`; reusable non-command skills are shared from `.agents/skills`; `AGENTS.md` guidance stays native; Setup-Machine also appends `[mcp_servers.*]` |
| Claude Code CLI | `~/.claude/` *(future)* | Deferred until confirmed usage |

Setup-Machine also performs a small search-tool preflight for `rg` (ripgrep). If `rg` is missing, the Windows script offers to install it via `winget`; the Unix script offers the first supported package-manager install path it detects. If ripgrep appears to be installed already but the current shell cannot resolve it yet, Setup-Machine warns that a new terminal is required.

### Repo-Layer Adapter Outputs

| Output File | CLI(s) | Content Strategy |
| --- | --- | --- |
| `AGENTS.md` | Copilot + Codex + Gemini (via bridge) + Claude Code | Canonical shared contract. Inline skills, conventions, workflows. |
| `.github/copilot-instructions.md` | Copilot only | Additive Copilot layer. No inline skills (machine install provides them). |
| `GEMINI.md` | Gemini *(transitional shim)* | Kept during migration to bridge. Inline skills. |

### Slash-Command / Skill Availability

| CLI | Status | Location |
| --- | --- | --- |
| Copilot CLI | Available | `~/.copilot/skills/<command>/` |
| Gemini CLI | Available via native custom commands | `~/.gemini/commands/<command>.toml` |
| Codex CLI | Available via `/skills` or `$skill` mention, not custom `/slash-command` syntax | `~/.codex/skills/<command>/` |
| Claude Code CLI | Deferred | MCP tools |

### Adding a New CLI

To add a new CLI runtime to GAL:

1. **Machine layer**: determine if the CLI has a global config dir; if yes, add a symlink or config step to both Setup-Machine scripts.
2. **Repo layer**: determine the CLI's canonical instruction file. If it is `AGENTS.md`, no code change needed — the repo adapter generator already writes it. Otherwise add a `Build-AdapterContent` / `build_adapter` call to both Sync-DevContext scripts.
3. **Settings bridge pattern**: if the CLI reads a configurable filename list (like Gemini's `context.fileName`), add a settings-write/merge step to Setup-Machine.
4. **Command surface layer**: if the CLI supports native commands, generate them from the same canonical command templates instead of maintaining a second workflow source.

## MCP Management

GAL now manages a repo-tracked MCP source of truth separately from skill installation.

| File | Scope | Role |
| --- | --- | --- |
| `mcp-servers.example.json` | tracked | Canonical MCP catalog and provider defaults |
| `mcp-servers.local.json` | local only | Optional enable/override layer for the current machine |
| `config.local.env` | local only | Secrets, paths, and local values referenced by the manifest |

Setup-Machine uses a merge strategy, not overwrite:

- VS Code: merges missing servers into `Code/User/mcp.json`
- Gemini CLI: merges missing servers into `~/.gemini/settings.json` under `mcpServers`
- Codex CLI: appends missing `[mcp_servers.<name>]` sections to `~/.codex/config.toml`

The provider config files remain user-owned. GAL only fills gaps from the manifest.

### Provider Differences

The same server may use different keys or transports per runtime.

- VS Code uses `mcp.json` with `servers.<name>` entries
- Gemini CLI uses `settings.json` with `mcpServers.<name>` entries
- Codex CLI uses `config.toml` with `[mcp_servers.<name>]` tables

This is why MCP is managed as a separate manifest layer rather than being embedded into skills.

### Current Policy

- Shared core servers such as Context7, fetch, filesystem, memory, Microsoft Learn, puppeteer, and image fetch are configured across VS Code, Gemini, and Codex when feasible.
- Provider-specific or auth-sensitive servers such as GitHub MCP, Chrome DevTools MCP, and Obsidian MCP can stay enabled only on the runtimes where the config is already validated.
- Claude Code is intentionally out of scope for the current merge flow.

Optional external CLI sidecars such as OpenCLI are documented separately in [opencli-routing.md](opencli-routing.md). They are skill-layer or execution-layer dependencies, not MCP manifest entries.

### External Tool Availability

GAL treats optional external CLIs and MCP servers as skill-layer routing choices, not as one global runtime mandate.

- Setup-Machine provisions GAL's adapters and MCP manifest wiring, but it does not guarantee that every optional external CLI sidecar is installed.
- Each external-tool skill should document a preferred path, a fallback path, and an explicit no-tool behavior.
- If a CLI is unavailable and the skill has a documented MCP or workspace fallback, the agent should use that fallback.
- If neither the preferred path nor the documented fallback is available, the agent should stop with a clear blocker unless the omitted step is a non-critical read-only check.
- Write paths and safety-critical validation should not silently degrade to a weaker substitute.

This keeps installer behavior minimal while making runtime tool failure explicit and predictable.

## Layer 1.5: Tool Installation Surface

Setup-Machine creates this effective topology.

| Source | Copilot Target | Gemini Target | Codex Target | Notes |
| --- | --- | --- | --- | --- |
| `agent/*.agent.md` | `~/.copilot/agents/` | — | — | Copilot-only custom agents |
| `skills/*/` | `~/.copilot/skills/` | `~/.agents/skills/` | `~/.agents/skills/` | Shared portable reusable skill set |
| `commands/*/` | `~/.copilot/skills/<command>/` | `~/.gemini/commands/<command>.toml` | `~/.codex/skills/<command>/` | One canonical command template, emitted into each runtime's native command surface |
| `<repo root>` | `~/.copilot/gal/` | `~/.gemini/gal/` | — | Stable GAL_ROOT symlink (not needed for Codex) |

## Generated Files

Setup-Machine also generates a small set of runtime files:

| Generated File | Purpose |
| --- | --- |
| `commands/*/SKILL.md` | Baked command skill with absolute GAL_ROOT (generated from each `SKILL.template.md`) |
| `~/.gemini/commands/*.toml` | Native Gemini slash commands generated from the baked command skill content |
| `~/.gemini/gal-context.md` | Aggregated `@file` imports for reusable non-command skills so Gemini can load shared GAL guidance without duplicating discovered command skills |

These files are generated because the tools need runtime-specific absolute paths, while the templates in the repo remain portable.

## Why GAL_ROOT Exists

`~/.copilot/gal/` and `~/.gemini/gal/` give installed command skills one stable way to reference the canonical repo.

That keeps runtime command prompts small and deterministic:

- skills do not need to infer repo location
- setup can bake paths once
- commands keep reading the same workflows, conventions, templates, and agents regardless of tool

## Cross-Machine Model

Each machine clones the same GAL repo and runs the same setup script for its platform.

```text
machine A                      machine B
-----------                    -----------
~/golem-agents-legion/         ~/golem-agents-legion/
~/.copilot/...                 ~/.copilot/...
~/.gemini/...                  ~/.gemini/...
        \                        /
         \---- git push/pull ----/
```

The methodology stays synchronized through Git. Machine-specific differences live in local tool configuration and model routing, not in duplicated copies of the methodology.

## Mac Mini Async Endpoint Baseline

The Mac Mini is planned as an always-on async endpoint, not as a second control plane.

Confirmed installed tooling on the Mac Mini:

- Gemini CLI
- Copilot CLI
- VS Code
- Codex CLI

Execution-plane implications:

- Gemini CLI is the only runtime currently intended to participate in automated remote dispatch.
- Copilot CLI, VS Code, and Codex CLI are available for interactive or manual work on the Mac Mini, but they are outside the current headless worker contract.
- The Mac worker path should use a bash entrypoint (`Start-GalWorker.sh`) rather than requiring PowerShell on macOS.
- For Apple Silicon local inference, prefer MLX-LM as the LOCAL lane. Ollama can remain an optional compatibility layer, but it is not the preferred baseline for the Mac Mini plan.
- The Mac Mini may additionally host Discord / Telegram bridge services for out-of-home task intake, but those bridges must feed the same bounded task contract instead of inventing a second workflow.

Suggested Apple Silicon local model allocation:

- Gemma 4: general background summarization, classification, and low-risk pre-processing
- Breeze 2: Traditional Chinese / Taiwan-specific wording, note cleanup, personal knowledge-base assistance, and private-text triage
- Gemini CLI: stronger headless worker path for tasks that need repo-aware output or more stable bounded execution

## Verification Checklist

After running Setup-Machine, verify:

1. `~/.copilot/gal/` and `~/.gemini/gal/` point to the GAL repo root.
2. `~/.copilot/skills/gal/`, `~/.gemini/commands/gal.toml`, and `~/.codex/skills/gal/` exist.
3. Shared `.agents/skills/` contains reusable skills only, not GAL command aliases.
4. Generated `SKILL.md` files no longer contain `{{GAL_ROOT}}`.
5. `~/.gemini/gal-context.md` exists and all import paths reference `.agents/skills`.

## Related Operational Sources

- [scripts/scripts.md](../scripts/scripts.md)
- [commands/commands.md](../commands/commands.md)
- [README.md](../README.md)
