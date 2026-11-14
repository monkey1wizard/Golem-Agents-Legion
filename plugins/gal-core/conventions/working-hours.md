# Working Hours Convention

All Golem agents MUST resolve working-hours behavior before performing any work.
This convention is referenced by every agent and enforced at the workflow level.

The working-hours boundary is a machine-local preference, not a tracked repo-wide requirement. It is disabled unless the local configuration explicitly enables it.

## Machine-Local Settings

Read these values from local configuration when available:

| Setting | Meaning |
| --- | --- |
| `WORKING_HOURS_ENABLED` | Enables working-hours behavior when `true` |
| `WORKDAY_START` | Start of the preferred workday in `HH:MM` |
| `WORKDAY_END` | End of the preferred workday in `HH:MM` |
| `WRAP_UP_TIME` | Reminder or shutdown-window start in `HH:MM` |
| `HARD_STOP_TIME` | Hard stop in `HH:MM` |

If `WORKING_HOURS_ENABLED` is missing or `false`, skip all working-hours behavior and proceed normally.

## Time Windows

| Zone | Purpose |
| --- | --- |
| **Working Hours** | Informational schedule from `WORKDAY_START` to `WORKDAY_END` |
| **After Hours** | Any time outside `WORKDAY_START` to `WORKDAY_END` before the configured stop boundary |
| **Wrap-up Time** | `WRAP_UP_TIME` — warn and offer `/gal wrap-up` once |
| **Hard Stop** | `HARD_STOP_TIME` — all agents refuse work |

## Rules by Time Window

### Working Hours Off

If `WORKING_HOURS_ENABLED=false` or unset, all agents operate normally. No restrictions.

### During Working Hours — Normal Operation

All agents operate normally. No restrictions.

### After Hours Before Wrap-up Time

After `WORKDAY_END`, agents may still proceed. This period is informational only. The shutdown ritual begins at `WRAP_UP_TIME`.

### From Wrap-up Time To Hard Stop — Shutdown Window

During the shutdown window:

- Allow minor wrap-up only.
- Offer `/gal wrap-up` at most once per invocation when the current repo is initialized for GAL.
- Do not run `/gal wrap-up` automatically.
- Only run `/gal wrap-up` after an explicit user confirmation such as `yes, run wrap-up`.
- If the repo is not initialized for GAL, do not offer `/gal wrap-up`; just give the working-hours reminder.

Use this reminder:

```text
It is past the configured Wrap-up Time. Time to wrap up.
I can run /gal wrap-up for this repo if you want, but I will only do that if you explicitly confirm.
```

### At Or After Hard Stop

**ALL agents** refuse to work, **except pipeline-phase execution** (see `## Pipeline Execution Exemption` below — `/gal pipeline` and its dispatched pipeline-phase golems are machine self-driving and not bounded by working hours). For interactive/chat work and direct golem calls, use this exact response:

```text
It is now past the configured Hard Stop. Per the owner's settings, work should stop now.
You can continue tomorrow from each repo's .dev/state.md.
```

No exceptions for interactive work. No `just one more thing.`
Do not describe this as a workflow gate, approval gate, or manual override requirement unless the user explicitly asks about working-hours policy.

## Implementation

This convention is enforced by each agent reading `conventions/working-hours.md` as part of their startup check. The time check uses the system clock and compares it against the configured time strings:

```powershell
Get-Date -Format HH:mm
```

## Override

If the user explicitly says `override working hours`, `skip working hours`, or `override curfew`:

- Allow work but print a single reminder: `Working-hours boundary temporarily overridden. Remember to rest.`
- Do NOT nag repeatedly after override is granted.
- Override expires at next invocation (does not persist).
- Do not mention override proactively in the Hard Stop refusal message.

## Pipeline Execution Exemption

The working-hours boundary protects the **user** from overwork. It bounds **interactive/chat work and direct golem calls** — not machine self-driving execution.

- **Interactive/chat + direct golem calls** are bounded as described above (Wrap-up offer and Hard Stop refusal). There is no diary gate or special wrap-up role.
- **`/gal pipeline` execution is exempt by default** — once started it keeps running across the Wrap-up Time and Hard Stop boundaries. There is **no opt-in flag**: a pipeline self-driving toward completion is not the user working late, so the boundary does not apply.
- **Dispatched pipeline-phase golems inherit the exemption** via the existing dispatch context: a golem invoked with `DISPATCH_KIND: pipeline-phase` (or a `PIPELINE_PHASE` marker) **skips its own working-hours refusal** and proceeds.
- **A direct interactive golem call** (no `DISPATCH_KIND: pipeline-phase` context) **stays bounded** — it still offers wrap-up / refuses at Hard Stop. The exemption must not leak to interactive calls.

Rationale: the boundary exists to protect the user from overwork; a machine self-driving an already-started pipeline is not user overwork. Interactive work keeps the time-boundary reminder without depending on any private diary system.

## Why This Exists

This is an optional self-imposed boundary to enforce healthy work habits. The agents serve the user, and when working hours are enabled they should protect the user from overwork without pretending the policy is repo-global.
