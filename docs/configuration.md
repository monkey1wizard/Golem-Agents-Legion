---
type: Guide
title: Configuration
description: Manage machine-local settings in `~/.gal/config/config.json`, including executor routing, host-subagent model tiers, plan language, MCP sources, the personal layer, and local-notes routing.
tags:
  - configuration
  - routing
  - mcp
  - personal-layer
status: stable
---

# Configuration

## Machine-Local Settings (`config.json`)

GAL stores machine-local settings in `~/.gal/config/config.json`. Values in this file apply exclusively to the current machine. Never commit machine-local values into version-controlled files, command templates, or source code. Recognized keys include `galSkills`, `planLanguage`, `memoryHarvest`, `executorRouting`, and `hostSubagentModels`. Working-hours behavior is not a `config.json` object. Agents apply the working-hours values described in [`working-hours.md`](../plugins/gal-core/conventions/working-hours.md) when local configuration enables them, and no crate under `crates/` reads those values. GAL detects plugin registration from each runtime's own configuration. Existing `pluginMode` properties are retired and ignored, and `gal doctor` reports them as stale keys. `schemaVersion` belongs in `plugins.lock.json` rather than `config.json`. `enabledPlugins` is a setting in GitHub Copilot's own `settings.json`, which GAL reads as registration evidence.

Deprecated keys (`devMode`, `galRoot`, and `secrets`) are ignored. If `gal doctor` detects `devMode` or `galRoot`, it emits a non-blocking warning. The `secrets` key is retired. It served a legacy MCP host writer, and GAL reads no credential data from this file.

### Primary Fields

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `executorRouting` | optional | object | Role-based executor routing grouped by consumer. Serves as the system's sole routing authority. |
| `galSkills` | optional | string | Absolute path to the local skills directory. This is the only remaining machine-local path substitution key. |
| `hostSubagentModels` | optional | object | Host-and-tier model overrides for host-subagent dispatch. See [Host-Subagent Dispatch Mode](#host-subagent-dispatch-mode). |
| `planLanguage` | optional | string | Default output language for `.dev/plans/*.md` and `.dev/research/*.md` when unspecified. Defaults to `en`. |
| `memoryHarvest` | optional | object | Provider memory harvesting configuration shaped as `{ enabled: boolean }`. Disabled by default. |
| `quickfix` | optional | object | Limits and the Stop-hook switch for the quickfix lane. See [Quickfix Lane](#quickfix-lane-quickfix). |

Core configuration does not manage external notes backends. For external notes specifications, see `plugins/gal-core/conventions/optional-capabilities.md`. Repository research outputs default to `.dev/research/`, and temporary working files reside in `.dev/tmp`.

## Quickfix Lane (`quickfix`)

The `quickfix` key sits at the top level of `~/.gal/config/config.json`. It sets the limits of the quickfix lane described in [Workflows](workflows.md#quickfix-lane). `gal quickfix-run`, `gal quickfix-check`, and `gal quickfix-hook` read it. A missing file or a missing key means the defaults below.

| Field | Type | Default | Description |
| --- | --- | --- | --- |
| `maxFiles` | non-negative integer | `3` | Most files one quickfix change may touch. |
| `maxLines` | non-negative integer | `50` | Most changed lines one quickfix change may hold. |
| `maxAttempts` | non-negative integer | `2` | Most attempts for the same failing commands at the same `HEAD`. |
| `hook` | boolean | `false` | When `true`, the host Stop hook compares the working tree with the snapshot stored for the same session at its previous Stop. An unchanged snapshot allows the stop. A changed snapshot makes the hook try to open an attempt. A successful open blocks the host stop so the host continues with the attempt. A refused open allows the stop. |

```json
{
  "quickfix": { "maxFiles": 3, "maxLines": 50, "maxAttempts": 2, "hook": false }
}
```

A malformed `quickfix` key is an error and has no fallback to the defaults. These cases are errors: a `quickfix` value that is not an object, a limit that is not a non-negative integer, a `hook` value that is not `true` or `false`, and any field name outside the four above.

## Repository Hygiene Checks

`gal finalize-check` includes `project-md-bound`, which reports whether `.dev/project.md` exceeds its 30,720 B normalized-LF UTF-8 advisory cap. `state-bound` checks `.dev/state.md`, whose size cap is 16,384 B, along with its bounded table shape. Hygiene findings are repairable conditions in these two bounded indexes. Supported source-document shape and path findings are also advisory. Integrity findings include problems in plans, prompts, receipts, research records, approval evidence, root adapters, unsupported state conflicts, and same-place conflicts. Hygiene findings are reported as `advisory` and do not block a receipt. Unreadable files and integrity findings remain `fail` conditions. `pass` and `advisory` count as passed receipt states, while `fail` and `not-run` block.

Before pipeline preflight and the finalize full-mode gate, GAL repairs hygiene findings in `.dev/project.md` and `.dev/state.md` with section-local edits, then reruns the same check once. It does not repair other files or continue repair after a gate fails in the same invocation. A remaining finding creates or updates one `Hygiene debt:` row in `.dev/state.md`, keyed by the active plan slug. GAL reports that debt in handback and finalize summaries and removes the row after a successful recheck. The `project-md-bound` cap and all finding, repair, restoration, debt, and integrity rules are defined in the single canonical [`.dev` hygiene and integrity convention](../plugins/gal-core/conventions/token-budget.md#dev-findings-hygiene-and-integrity).

When `.dev/state.md` is unmerged alongside other paths, `gal state-merge` can resolve supported disjoint keyed rows or plain regions while leaving the other unmerged paths untouched. Same-place conflicts, authority-bearing conflicts, and conflicts outside supported state places remain integrity stops. See the canonical convention for conflict reporting and translation provenance: translated workflow and configuration guides follow their English source documents, with source and language recorded in translation front matter.

## Executor Routing (`executorRouting`)

### Host-Subagent Dispatch Mode

Dispatch mode defaults to `routed` when a route is present for the role. A current-session request pins `host-subagent` and overrides a valid route. Finalize review always uses its route. An invalid route stops the dispatch when a host is pinned or during finalize review. In an unpinned ordinary dispatch, an invalid route falls back to the permissive loader, and the dispatch reports `no-routing` when the role has no usable entry. With no route and a pinned host, the mode is `host-subagent`. With no route and no pinned host, the mode is `text`. The emitted `DISPATCH_MODE` values are `offload` for routed dispatch, `host-subagent`, and `text`. The exact phrases, host selection, attempt lifecycle, authority rules, and PASS requirements are defined in the [pipeline skill](../plugins/gal-core/commands/gal-pipeline/SKILL.template.md#dispatch-mode).

The host capability check must confirm that the selected host can spawn and receive completion evidence for subagents. Claude Code uses `--host claude`. A capable Codex Desktop or Codex CLI uses `--host codex`. Capability evidence is separate for Codex Desktop and Codex CLI. A live acceptance run passed in Claude Code, in Codex CLI 0.162.1, and in Codex Desktop 26.1013.31256, where every Codex phase spawned a fresh default subagent with `fork_turns: "none"` and the requested model and effort. A capability failure or invalid host is rejected without silent substitution.

The built-in model map uses Haiku for Claude light, Sonnet for standard, and Opus for deep, with Fable as an explicit supported deep alternative. Codex uses Luna for light, Sol for standard, and Astra for deep. Model and effort availability depends on the host and account. A deep tier is reached only through a reasoned manual override, never through task content. Explicit `inherit` is a degraded opt-out. It records that model-tier policy was not applied and does not satisfy model-tier acceptance. Never silently substitute inherit when model or effort selection is unavailable.

`hostSubagentModels` is a top-level `config.json` key that overrides the host-and-tier mapping. Its shape is `{"codex"|"claude": {"light"|"standard"|"deep": {"model": "...", "effort": "..."}}}`. Its constraints are:

- Only the hosts `codex` and `claude` are accepted. Only the tiers `light`, `standard`, and `deep` are accepted. Any other host or tier key is an error.
- A Codex entry's `model` must be `gpt-6-luna`, `gpt-6.1-sol`, or `gpt-6-astra`. Its `effort` is required and must be `medium`, `high`, or `xhigh`.
- A Claude entry's `model` must be `haiku`, `sonnet`, `opus`, or `fable`. Its `effort` is optional. When set, it must be `low`, `medium`, `high`, `xhigh`, or `max`.
- An `inherit` model takes no `effort`.
- An unsupported model or effort, a value that is not an object, or a malformed entry is an error. The attempt stops and does not fall back to the default map.

The `OPEN` step is the sole resolver. It records the resolved model, effort, tier, tier source, and override reason. The orchestrator uses the `model=` value from `HOST_SUBAGENT_ATTEMPT:` as the spawn model. It passes `--tier` with `--tier-reason` only for a reasoned manual override. Under guarded continuation, use computed tiers without conversational overrides.

### Host-Subagent Tier Rules

Task content, declared files, crate count, protected paths, and dependency count do not affect the tier. The computed tiers are:

- **Implement**: `light` for every task. A retry raises it to `standard`, as described below.
- **Test**: one tier below the accepted implement tier, with a floor of `light`.
- **Audit**: the highest of `standard`, the accepted implement tier, and the accepted test tier.
- **Retry**: any prior failed attempt for the same task and phase raises an auto-computed `light` to `standard`. More failures never raise an auto-computed tier above `standard`. A tier that is already `deep`, from accepted prerequisite evidence or a manual override, stays `deep`.
- **Tierless evidence**: accepted routed or text evidence with no recorded tier counts as `standard`, with source `prerequisite-tier-unknown`. A tierless implement prerequisite therefore gives test `light`, and audit keeps the `standard` floor. Tierless host-subagent evidence stays invalid.
- **Finalize review**: without an override, host-subagent finalize review bypasses `hostSubagentModels` and resolves model `inherit` with no effort override. The attempt log records tier `standard`, source `finalize-review`, and `model_tier_applied: no`. Routed finalize review keeps the AUDITOR route unchanged. A manual host-subagent override resolves through the model map. A lowering override stays invalid.
- **Reasoned override**: `--tier deep --tier-reason <reason>` is permitted in every host-subagent phase when the reason is non-empty. The limits on lowering overrides are unchanged.
- **Advisory**: when an earlier attempt for the same task and phase failed at `standard`, and the selected tier is still `standard`, `OPEN` prints one stable `HOST_SUBAGENT_ADVISORY:` line recommending `--tier deep --tier-reason <reason>`. The advisory never changes the selected tier or the `HOST_SUBAGENT_ATTEMPT:` record.

### Host-Subagent Spawn Contract

The SPAWN instruction sends the published manifest `spec_path`, attempt ID, phase, task, allowed files, receipt path, and write-back instructions. It does not send the spec body. The message tells the subagent to read the complete spec file before acting. CLOSE still verifies `spec_sha256` from the attempt manifest. The same instruction requires a final reply of at most 15 lines: the outcome and receipt path, plus the first error with its file and line range on failure. The reply carries no file contents, diff, or passing-test output. Acceptance evidence stays in the receipt and attempt log.

### Preflight Dispatch Flags

`gal pipeline-preflight` pins the dispatch mode with these flags:

- `--dispatch routed` or `--dispatch host-subagent` sets the pinned mode. The default is `routed`. `--dispatch host-subagent` requires `--host`.
- `--host claude` or `--host codex` pins the host. Any other value is rejected.
- `--host-session <host>:<session-id>` names the orchestrator session. Preflight verifies that the session matches the pinned host and the working directory. The reference must use the pinned host. `--host-session` is not accepted with `--terminal-reverify`. Without it, the recorded orchestrator binding is unavailable.

### Consumer Groups and Role Entries

The `executorRouting` object organizes agent roles into three consumer groups:

- **`planning`**: Review group containing `ARCHITECT`, `ANALYST`, `DESIGNER`, and `RELEASER`. This group feeds only the Codex native-agent projection. See [Planning Group Scope](#planning-group-scope).
- **`pipeline`**: Dispatch group containing `CODER`, `TESTER`, and `AUDITOR`. This group uses headless dispatch.
- **`research`**: Numbered research worker keys. This group uses headless dispatch. See [Research Routing](#research-routing).

All three groups inherit defaults from a shared `executors` block, which functions as the single routing authority. Legacy flat routing schemas are retired, are no longer parsed, and produce warnings. Standalone routing configuration files are no longer supported.

The literal executor names are `claude`, `codex`, `opencode`, `copilot`, and `agy`. An `executor` value must be one of these names or the name of a `combinations` entry.

Keys that begin with `_` are comments. GAL skips them silently in `combinations`, in each consumer group, in `research`, and at the top level under `executorRouting`. The `executors` block does not skip them. A flat role key directly under `executorRouting` is retired and produces a warning. Any other unrecognized top-level key also produces a warning.

### Role Entry Schema

A role entry sits under its group, for example `executorRouting.pipeline.CODER`.

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `executor` | required | string | A literal executor name or a `combinations` name. |
| `model` | optional | string | Overrides the combination and `executors` default model. |
| `effort` | optional | string | Reasoning effort. Overrides the combination and `executors` default effort. |
| `timeoutSecs` | optional | positive integer | Overrides the default timeout. A value of `0` is ignored with a warning. |
| `sshTarget` | optional | string | Remote SSH target. Set it with `remoteWorkdir`. |
| `remoteWorkdir` | optional | string | Remote working directory. Set it with `sshTarget`. |

Role keys are case-sensitive:

- `pipeline` accepts `CODER`, `TESTER`, and `AUDITOR`.
- `planning` accepts `ARCHITECT`, `ANALYST`, `DESIGNER`, and `RELEASER`.
- `research` accepts only numbered worker keys, as described in [Research Routing](#research-routing).

A role placed in the wrong group, or an unknown role, is skipped with a warning.

### Planning Group Scope

The `planning` group applies only to the four review roles `ARCHITECT`, `ANALYST`, `DESIGNER`, and `RELEASER`. Their keys must be uppercase, as in `ARCHITECT`. The slash-command names stay lowercase. During Codex agent projection, GAL looks up the `model` and `effort` that resolve for each role and writes them into the projected Codex agent definition. This lookup does not launch an executor and does not offload work.

- A direct `/gal <role>` call (isolated mode) emits `ROUTING_SCOPE: codex-native-projection-only` and a `ROUTING_NOTE`. The note says that planning configuration feeds Codex native-agent projection only, that no executor is launched, and that the host decides the actual model. This text is a static contract statement. It does not prove that any configuration was applied.
- The `executor` key in a `planning` role is still read. It names the `combinations` entry or the `executors` entry whose `model` and `effort` the lookup uses. Precedence is the inline role value, then the combination, then the `executors` default. A role with no `planning` entry gets neither value, so Codex inherits its own model. The key does not choose a process to launch.
- The `timeoutSecs`, `sshTarget`, and `remoteWorkdir` fields have no effect on planning roles, because no headless process runs for them. Set these fields only for `pipeline` and `research` roles.
- GAL does not promise that runtimes other than Codex apply the resolved `model` or `effort`. Those runtimes keep their own native-subagent behavior, with or without planning configuration.
- In-context consultation through `/<role> discuss` runs in the active session and launches no executor.

### Finalize Review Route

The plan-level finalize review runs through `gal pipeline <execution-prompt> --phase finalize-review`. It uses the existing `pipeline.AUDITOR` route and adds no new role or configuration key.

- If the config file, the `executorRouting` key, the `pipeline` group, or the `AUDITOR` entry is genuinely absent, dispatch reports `no-routing`. This permits the in-process review marked `Review Independence: DEGRADED_SAME_RUNTIME`. Explicit host-subagent mode with a passing host capability check is the documented separate review exception.
- If the config file is unreadable or is not valid JSON, dispatch reports `routing-invalid` and stops. The same result applies when `executorRouting` or `pipeline` is not an object, or when the `AUDITOR` entry is malformed, has an empty `executor`, resolves to no model, or sets only one of `sshTarget` and `remoteWorkdir`. GAL does not fall back to the same runtime in these cases. For this phase, GAL does not inspect the entries of other roles.
- A missing executor, a launch failure, a timeout, a nonzero exit, a missing or empty receipt, or missing provider session evidence also stops the review. None of these cases falls back to the same runtime.
- A successful review needs a fresh non-empty receipt, an attempt log that ends in `completed`, and provider session evidence. It does not need a product diff.

Other dispatches keep their existing behavior when no host is pinned. For them, an unreadable or malformed config still degrades to `no-routing`. A pinned host stops instead.

### The `combinations` Registry

The `executorRouting.combinations` registry defines reusable executor configurations. Each entry specifies an `executor` along with optional overrides for `model`, `effort`, `timeoutSecs`, `sshTarget`, and `remoteWorkdir`. The `executor` of a combination must be a literal executor name, not another combination. A combination name cannot equal a literal executor name. Invalid combinations are rejected with a warning. For `planning` roles, only `model` and `effort` are read. Roles can reference a combination by name in their `executor` property, overriding combination values field by field. Note that `sshTarget` and `remoteWorkdir` must always be provided together.

### The `executors` Defaults Block

Entries in `executorRouting.executors` accept either string shorthands or full configuration objects:

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `model` | required | string | Default model identifier for the executor. |
| `effort` | optional | string | Default reasoning effort setting. |
| `timeoutSecs` | optional | positive integer | Execution timeout in seconds. A value of `0` is ignored with a warning. |

When resolving settings for a role, fallback order is deterministic:
**Inline role setting → Combination base → `executors[final.executor]` → `None`**.
The model resolution chain defaults to an empty string if unconfigured. If a role explicitly sets `timeoutSecs: 0`, timeout resolution yields `None` rather than falling back to defaults.

### Research Routing

The `research` group uses headless dispatch similarly to `pipeline`. In contrast, the `planning` group is verification-only and never invokes headless dispatch. Research workflows utilize three parallel workers: `RESEARCHER#0`, `RESEARCHER#1`, and `RESEARCHER#2`. The group key `"1"` routes `RESEARCHER#1`, and the key `"2"` routes `RESEARCHER#2`. Key `"0"` is skipped with a warning, because worker `#0` is always a native subagent. A role-name key such as `RESEARCHER` is skipped with a warning. Keys above `2` load, but no research dispatch reads them. A worker without a `research` entry uses the `executors.agy` default when one is set. Otherwise it runs as a native subagent. These workers run in isolation without mutual visibility. Once all workers finish, the orchestrator synthesizes findings directly. This process does not use an intermediate verification dispatch step, does not use a `verify` key in `executorRouting`, and routes only across the selected worker instances.

### Remote Execution

Remote execution parameters are configured per role inside `executorRouting` without auxiliary files. Roles in the `pipeline` and `research` groups specify `sshTarget` and `remoteWorkdir` concurrently to designate SSH endpoints and repository working directories. Roles in the `planning` group launch no headless process, so these fields do not apply to them. Because these settings represent sensitive machine-local infrastructure details, never share them across repositories.

Credentials and secrets must never be placed in `config.json`. GAL does not extract credentials from configuration files.

### Codex Approval and Sandbox Settings

Modify `config.toml` to configure Codex approval policies and sandbox settings in Codex, or select them through Codex Desktop task modes. Command exceptions are configured separately in `~/.codex/rules/*.rules`. These settings operate independently from GAL `executorRouting` and `timeoutSecs` settings. GAL never modifies Codex host settings. A matched `allow` rule runs a trusted outer RTK or GAL command outside the parent sandbox without prompting. This rule does not change the sandbox applied to a dispatched child process. For tested configurations, rule examples, validation, recovery, and rollback, see [Codex Pipeline Execution Setup](setup.md#codex-pipeline-execution-setup).

During headless dispatch, GAL launches child Codex processes with `--dangerously-bypass-approvals-and-sandbox`. The child runs without the Codex sandbox, at the same full-access trust level as the other executors, because the Codex Windows sandbox currently fails before it runs any command. See [setup.md](setup.md#known-issue-the-codex-windows-sandbox-fails). The parent Codex session's approval mode does not change how the child is launched. Denied operations are never treated as authorized. Unobservable permission states remain classified as unknown.

## Machine-Local Path Placeholders

Git filter operations (`gal clean` and `gal smudge`, detailed in [workflows.md](workflows.md)) strip machine-local paths using placeholders:

| Placeholder | Definition | Application |
| --- | --- | --- |
| `<GAL_SKILLS>` | Absolute path from the `galSkills` key. | Applied by the `gal clean` and `gal smudge` git filters. |

Path substitution applies exclusively to `galSkills`. Other configuration keys do not undergo placeholder replacement.

## Model Context Protocol (MCP) Servers

GAL aggregates MCP servers from two distinct sources, merging them into the canonical root at `~/.gal/plugins/gal/` during `gal refresh`:

| Source | Location | Description |
| --- | --- | --- |
| **Core** | `plugins/gal-core/mcp.json` | Version-controlled servers required by GAL core. Kept empty by default. Entries are added only when core functionality requires them. |
| **Personal** | `~/.gal/local/mcp.json` | User-defined servers local to the host machine. The presence of this file acts as the enablement trigger without extra configuration flags. |

Both files use the standard `"servers"` dictionary schema.

### Merge and Generation Flow

Running `gal refresh` processes MCP servers through the following sequence:

1. Translates the core `mcp.json` `"servers"` dictionary into `~/.gal/plugins/gal/.mcp.json` under the `"mcpServers"` key.
2. If `~/.gal/local/mcp.json` exists, merges its server definitions into the `"mcpServers"` dictionary. If a personal server name conflicts with a core server, the personal definition is ignored and the core configuration takes precedence. If the personal file is missing or invalid JSON, GAL preserves core servers and continues execution cleanly.
3. Writes the combined server list to `~/.gal/plugins/gal/mcp.json` conforming to the Agent Plugins standard format. In this file, `"type": "http"` is rewritten to `"streamable-http"`. Other types, including `stdio` and `sse`, pass through unchanged. The `.mcp.json` file keeps `"http"`.

Server configurations pass through without path placeholder substitution. The only change is the `type` rewrite in step 3. Environment variables in server configs (`${ENV_VAR}`) are resolved dynamically by MCP host runtimes during process launch. Define these variables within the host shell environment rather than in `config.json`.

Configuration overrides are not supported via auxiliary files. To modify core server parameters, edit `plugins/gal-core/mcp.json` and run `gal dev-apply` from the checkout. To modify personal servers, edit `~/.gal/local/mcp.json` and rerun `gal refresh`. Runtimes like Claude Code require a restart or plugin reload to register configuration updates.

The personal MCP layer is merged during local `gal refresh` or `gal dev-apply` operations. Build outputs for public marketplace snapshots omit personal configurations entirely.

### Runtime Support Matrix

GAL does not inject MCP server configurations into external application directories, nor does it edit third-party settings files. Runtimes discover servers when loading GAL as a plugin:

| Runtime | Configuration Target | Integration Status |
| --- | --- | --- |
| Claude Code | `.mcp.json` | Supported natively. |
| OpenAI Codex | `.mcp.json` | Supported natively. |
| GitHub Copilot | `mcp.json` | Supported natively. |
| Google Antigravity | None | Not officially verified within plugin containers. |
| opencode | None | Not supported. opencode does not load Markdown plugins, and file projection does not include MCP configurations. |

"Supported natively" indicates GAL produces the configuration file expected by the runtime plugin loader. Actual loading depends on runtime support and requires completing registration steps in [setup.md](setup.md).

GAL never modifies host-managed configuration files such as `claude_desktop_config.json`, `~/.copilot/mcp-config.json`, `~/.codex/config.toml`, `opencode.json`, or `mcp_config.json`. These files remain user-managed. Configure application-wide tools directly in the respective runtime configuration file.

### Example: Custom MCP Server

The example below illustrates registering custom MCP servers via `~/.gal/local/mcp.json`. Multiple instances of a single tool can be registered with different environment variables. The `postgres-mcp` package runs via `uvx`:

```json
{
  "servers": {
    "postgres-app": {
      "type": "stdio", "command": "uvx",
      "args": ["postgres-mcp", "--access-mode=restricted"],
      "env": { "DATABASE_URI": "${POSTGRES_MCP_APP_URI}" }
    },
    "postgres-analytics": {
      "type": "stdio", "command": "uvx",
      "args": ["postgres-mcp", "--access-mode=restricted"],
      "env": { "DATABASE_URI": "${POSTGRES_MCP_ANALYTICS_URI}" }
    }
  }
}
```

Save this content to `~/.gal/local/mcp.json`, export `POSTGRES_MCP_APP_URI` and `POSTGRES_MCP_ANALYTICS_URI` in your shell, and run `gal refresh`. Both database servers will appear in generated plugin manifests.

## Plan Language Configuration (`planLanguage`)

The `planLanguage` key specifies the default language for generated `.dev/plans/*.md` and `.dev/research/*.md` files when unprompted. The resolution hierarchy is:
**Explicit user instruction → `planLanguage` setting → Prompt language detection → Default `en`**.
Project-level documentation policy is controlled separately via `PROJECT_LANGUAGE` in repository metadata. Execution prompts (`.dev/plans/*.prompt.md`) remain strictly in English to maintain deterministic multi-model performance.

Setting `planLanguage` to a non-English language enables a three-layer plan workflow:

1. **English technical draft** (`.dev/plans/<slug>.en.md`): Defines canonical technical specifications.
2. **Localized working plan** (`.dev/plans/<slug>.md`): Editable localized file for human review. Section headers, file paths, task identifiers, and status verdicts remain in English, while descriptive text is localized.
3. **Execution prompt** (`.dev/plans/<slug>.prompt.md`): Final English compilation for model execution.

Manual edits made to localized plans are preserved. When subsequent planning commands execute, they pause at a read-only reconciliation step to backport manual revisions into the English draft. Manual edits are never overwritten. When `/plan-to-prompt` compiles the final execution prompt, the intermediate draft is deleted, leaving two persistent plan files. Setting `planLanguage: "en"` bypasses translation layers entirely.

## Usage Accounting Inputs

Usage accounting reads two optional per-run files when you run `gal usage collect`. Neither file belongs to `~/.gal/config/config.json`, and neither file carries token or cost counters. The contract is [`usage-accounting.md`](../plugins/gal-core/conventions/usage-accounting.md).

```text
gal usage collect --run <run-id> [--binding <path>] [--rates <path>]
```

### Binding File (`--binding`)

The `--binding` flag is optional when preflight recorded the orchestrator binding. Pass `--host-session <host>:<session-id>` to `gal pipeline-preflight`, and preflight verifies that session against the pinned host and the working directory. When it verifies, `gal usage collect --run <run-id>` without `--binding` collects from that recorded binding. When preflight recorded no binding, the collection fails and names the reason, and `--binding` is then required.

The binding file names the run, the host session, and the source window. It is a JSON object with exactly these fields: `run`, `host_session`, `billing_mode`, `role`, `window`, and an optional `cursor`.

```json
{
  "run": {
    "version": 1,
    "run_id": "run-a",
    "canonical_worktree": "C:/work/repo",
    "plan_scope": "plan-a",
    "host_session": "codex:sess-a",
    "host_window": "window-a"
  },
  "host_session": "codex:sess-a",
  "billing_mode": "standard",
  "role": {"kind": "orchestrator"},
  "window": {"started_at": "2026-10-20T00:00:00.000Z"}
}
```

- Any other field fails the collection. This rule covers counter fields such as `input_tokens` and assessment fields.
- The `run.run_id` value must equal the `--run` value.
- When `host_session` differs from the host session in the run identity, the collection fails as a rebind conflict. It fails before any source is read.

### Rate Set File (`--rates`)

The rate file is an owner-supplied rate set. The schema version is `1`. GAL does not fetch rates.

```json
{
  "schema_version": 1,
  "rate_set_id": "rates-a",
  "rates": [
    {
      "provider": "codex",
      "model": "gpt-x",
      "billing_mode": "standard",
      "category": "input",
      "unit": "per_million_tokens",
      "price": "2.5",
      "money_unit": {"currency": "USD"}
    }
  ]
}
```

- Rates key on provider, observed model, billing mode, and unit. A configured routing name never replaces the observed model.
- Prices are decimal strings. A negative price fails the parse. A request price cannot price token counters.
- Without a rate file, every rate-derived estimate stays unknown, for example `estimate: unknown`.
- A changed rate set creates a new snapshot. The earlier snapshot does not change. A changed rate set changes only the estimates. Unit totals, such as 160 for a retried task and 200 for an inclusive parent, do not change.
- `gal usage report` reads stored records only. `gal usage assess` takes an assessment input and no binding or rate file.

## Memory Harvesting (`memoryHarvest`)

Memory harvesting captures operational lessons learned by models during active sessions, persisting them into repository files to retain context across session restarts:

- **Disabled by default**: Enable by setting `config.json#memoryHarvest.enabled: true`. If omitted or set to `false`, the feature remains inactive.
- **Bounded exploration**: GAL never parses, indexes, or scans global chat histories or local session dumps for candidates.
- **Context-bound and de-identified**: Candidate lessons must reference concrete repository files. Candidates are sanitized to strip raw conversation logs, secrets, machine-local paths, and personal notes.
- **Human approval required**: Harvested candidates can only be reviewed and accepted during `/gal wrap-up`. Unapproved candidates are discarded without touching disk. Approved entries are recorded in the active plan's handoff notes as advisory task context.
- **Promotion to documentation**: Memory harvest approval does not automatically alter documentation. Approved notes require independent verification during `/gal finalize` before promotion into `docs/`.

## Local Customizations

### Personal Directory Root (`~/.gal/local/`)

User-specific assets reside in `~/.gal/local/`. These assets are rendered into the canonical root and projected across all active runtimes. GAL reads from `local/skills/`, `local/mcp.json`, and `local/conventions/`, but **never modifies, creates, or deletes files inside user-managed directories**. Multi-machine file synchronization is managed by the user.

#### Directory Layout

```text
~/.gal/local/
  skills/
    <skill-name>/
      SKILL.md          ← Personal skill definition (read-only for gal)
  mcp.json              ← Personal MCP servers (read-only for gal)
  conventions/
    <lang>.md           ← Personal language coding convention (read-only for gal)
```

#### Activation

Activation is determined strictly by file existence without requiring configuration flags. Placing files within `~/.gal/local/` activates personal overlays. If directories or files are missing, output generation matches default core behavior exactly.

#### Precedence and Collisions

- Personal skills are evaluated after core skills. If a personal skill shares an identifier with a core skill, the personal version is skipped and the core skill takes precedence.
- Personal MCP servers are merged into the canonical `.mcp.json`. Name collisions resolve in favor of core definitions.
- Running `gal doctor` reports personal skill counts, active convention files, and any definitions skipped due to name collisions.

### Coding Style Conventions

Core GAL templates do not impose personal coding styles. Custom styling can be applied to target repositories through three mechanisms:

#### Source 1: Personal Convention Files (Repository-Wide)

Copy `plugins/gal-core/templates/csharp-convention.example.md` to `~/.gal/local/conventions/csharp.md` and customize rules. Accepted file stems are strictly limited to `csharp`, `typescript`, or `go`. The `typescript` file applies to projects listing either TypeScript or JavaScript in their `Language` setting, but the file stem must remain `typescript`. If you previously relied on generic naming, rename files to match approved stems. When `gal render-adapters` runs in a repository with a matching `Language` value in `.dev/project.md`, rules are injected into adapter instructions similarly to built-in conventions.

#### Source 2: Installed Plugin Discovery (Zero Configuration)

If your coding runtime contains official language plugins, run `gal render-adapters` in the target repository. GAL matches plugin metadata against the repository's `Language` field, rendering a **Detected Language Skills** instruction block that references the installed plugin. GAL never copies external plugin source code, nor does it install, update, or remove plugins. Run `gal render-adapters` after installing new runtime extensions to update detection references.

#### Source 3: Personal Skills (On-Demand Loading)

Create a custom skill under `~/.gal/local/skills/<name>/SKILL.md`. Claude Code requires a restart to index new skill directories. Other runtimes detect files on their subsequent read cycle. These skills load only when explicitly invoked by name, avoiding global context pollution.

#### Selecting a Customization Source

For standard setups with official plugins installed, choose Source 2 for zero-maintenance integration. Choose Source 1 when you need consistent, enforced team conventions across projects. Use Source 3 for specialized, ad-hoc agent workflows.

#### Fallback Behavior

If a repository's `Language` metadata matches no convention file or plugin, generated adapters omit language convention blocks. This represents normal operational fallback rather than an error condition.

#### Disabling Automatic Conventions

To disable automatic convention injection from Sources 1 and 2, add the row `| Personal Conventions | off |` to the `Tech Stack` table in `.dev/project.md`. Public repositories should enable this setting to prevent machine-local configurations from leaking into committed adapters. Source 3 skills remain available for on-demand use.

### Local Command Overlays (`SKILL.local.md`)

To customize command skills locally, create `plugins/gal-core/commands/<command>/SKILL.local.md`:

- The file is gitignored and treated as user-owned configuration.
- During build routines, contents of `SKILL.local.md` are appended to the generated `SKILL.md`.
- Never edit `plugins/gal-core/commands/<command>/SKILL.md` directly. That file is regenerated automatically during projection.
- `SKILL.local.md` supplies supplementary prompt instructions only. Do not include front matter blocks.

### External Notes Integration (`local-notes`)

Connecting external personal notes is entirely optional. GAL maintains strict separation between repository state and private note vaults. Core functionality does not depend on external note stores. The portable specification is detailed in [`optional-capabilities.md`](../plugins/gal-core/conventions/optional-capabilities.md). Notes integration is disabled by default and engages only after local backend availability is verified.

If a notes backend is unavailable, unconfigured, or offline, the system degrades gracefully to standard repository workflows without throwing errors. Only explicitly connected backends can perform reads or writes at authorized workflow checkpoints. Notes integrations are never prerequisites for core features. Repositories without note backends function identically to connected repositories, minus access to private vault context.

Supported backend implementations include Obsidian (`coddingtonbear/obsidian-local-rest-api`), Logseq (`ergut/mcp-logseq`), Joplin (`joplin-mcp`), generic Markdown directories (`vault-mcp`), and `SeekLink` for CJK-focused retrieval. Feature parity varies across implementations.

Repository research documents are stored in `.dev/research/` by default.

### In-Context Role Dispatch

The architect, analyst, designer, and releaser command skills use `gal consult-script <role>` for in-context consultation. This dispatch is local and does not use the SSH pipeline executor route. The retired `/gal discuss <role>` form returns an error with the standalone command to use.
