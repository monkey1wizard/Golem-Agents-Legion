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
| `OBSIDIAN_DIARY_DIR` | Diary location used to check whether today's diary exists |

If `WORKING_HOURS_ENABLED` is missing or `false`, skip all working-hours behavior and proceed normally.

## Time Windows

| Zone | Purpose |
| --- | --- |
| **Working Hours** | Informational schedule from `WORKDAY_START` to `WORKDAY_END` |
| **After Hours** | Any time outside `WORKDAY_START` to `WORKDAY_END` before the configured stop boundary |
| **Wrap-up Time** | `WRAP_UP_TIME` — warn, offer `/gal wrap-up`, and redirect to the after-hours owner if the diary is missing |
| **Hard Stop** | `HARD_STOP_TIME` — all agents refuse work |

## Rules by Time Window

### Working Hours Off

If `WORKING_HOURS_ENABLED=false` or unset, all agents operate normally. No restrictions.

### During Working Hours — Normal Operation

All agents operate normally. No restrictions.

### After Hours Before Wrap-up Time

After `WORKDAY_END`, agents may still proceed. This period is informational only. The shutdown ritual begins at `WRAP_UP_TIME`.

### From Wrap-up Time To Hard Stop — Shutdown Window

**Check**: Does today's diary exist at `<OBSIDIAN_DIARY_DIR>/YYYYMMDD_Work_Diary.md`?

The after-hours owner is the single Obsidian-writing agent, currently `@golem-notewriter`.

| Diary exists? | Agent is after-hours owner? | Action |
| --- | --- | --- |
| No | No | **BLOCK**. Print warning, redirect to the after-hours owner |
| No | Yes | Proceed with shutdown ritual |
| Yes | No | Allow **minor wrap-up only** (commit, push, save). Offer `/gal wrap-up` once, but run it only if the user explicitly agrees |
| Yes | Yes | Allow supplement or archive tasks only |

**Block message** (for non-owner agents):

```text
Past the configured Wrap-up Time and today's diary is not yet written.
Call the after-hours owner to complete the shutdown ritual before continuing other work.
```

**Wrap-up offer** (for non-owner agents when today's diary already exists):

```text
It is past the configured Wrap-up Time. Time to wrap up.
I can run /gal wrap-up for this repo if you want, but I will only do that if you explicitly confirm.
```

During the shutdown window:

- Offer `/gal wrap-up` at most once per invocation when the current repo is initialized and today's diary already exists.
- Do not run `/gal wrap-up` automatically.
- Only run `/gal wrap-up` after an explicit user confirmation such as `yes, run wrap-up`.
- If the repo is not initialized for GAL, do not offer `/gal wrap-up`; just give the working-hours reminder.

### At Or After Hard Stop

**ALL agents** (including the after-hours owner) refuse to work. Use this exact response:

```text
It is now past the configured Hard Stop. Per the owner's settings, work should stop now.
You can continue tomorrow from each repo's .dev/state.md.
```

No exceptions. No `just one more thing.`
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

## Why This Exists

This is an optional self-imposed boundary to enforce healthy work habits. The agents serve the user, and when working hours are enabled they should protect the user from overwork without pretending the policy is repo-global.
