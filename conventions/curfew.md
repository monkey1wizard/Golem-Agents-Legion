# Curfew Convention

All Golem agents MUST check the current time before performing any work.
This convention is referenced by every agent and enforced at the workflow level.

## Time Zones

| Zone | Purpose |
| --- | --- |
| **Soft curfew** | 22:00 — warn, offer `/gal wrap-up`, and redirect to `@golem-scribe` if the diary is missing |
| **Hard curfew** | 23:00 — all agents refuse work |

## Rules by Time Window

### Before 22:00 — Normal Operation

All agents operate normally. No restrictions.

### 22:00–23:00 — Shutdown Window

**Check**: Does today's diary exist at `10_Projects/Work_Journal/YYYYMMDD_Work_Diary.md`?

| Diary exists? | Agent is @golem-scribe? | Action |
| --- | --- | --- |
| No | No | **BLOCK**. Print warning, redirect to `@golem-scribe` |
| No | Yes | Proceed with shutdown ritual |
| Yes | No | Allow **minor wrap-up only** (commit, push, save). Offer `/gal wrap-up` once, but run it only if the user explicitly agrees |
| Yes | Yes | Allow supplement or archive tasks only |

**Block message** (for non-scribe agents):

```text
⏰ Past 22:00 — today's diary is not yet written.
Call @golem-scribe to complete the shutdown ritual before continuing other work.
```

**Wrap-up offer** (for non-scribe agents when today's diary already exists):

```text
It is past 22:00. Time to wrap up.
I can run /gal wrap-up for this repo if you want, but I will only do that if you explicitly confirm.
```

During the shutdown window:

- Offer `/gal wrap-up` at most once per invocation when the current repo is initialized and today's diary already exists.
- Do not run `/gal wrap-up` automatically.
- Only run `/gal wrap-up` after an explicit user confirmation such as "yes, run wrap-up".
- If the repo is not initialized for GAL, do not offer `/gal wrap-up`; just give the curfew reminder.

### After 23:00 — Hard Curfew

**ALL agents** (including @golem-scribe) refuse to work. Use this exact response:

```text
It is now past 23:00. Per the owner's settings, work should stop now.
You can continue tomorrow from each repo's .dev/state.md.
```

No exceptions. No "just one more thing."
Do not describe this as a workflow gate, approval gate, or manual override requirement unless the user explicitly asks about curfew policy.

## Implementation

This convention is enforced by each agent reading `conventions/curfew.md` as part of their startup check. The time check uses the system clock:

```powershell
(Get-Date).Hour
```

## Override

If the user explicitly says "override curfew" or "skip curfew":

- Allow work but print a single reminder: "Curfew temporarily overridden. Remember to rest."
- Do NOT nag repeatedly after override is granted
- Override expires at next invocation (does not persist)
- Do not mention override proactively in the hard-curfew refusal message

## Why This Exists

This is a self-imposed boundary to enforce healthy work habits. The agents serve the user — and part of serving well is protecting the user from overwork.
