# Curfew Convention

All Golem agents MUST check the current time before performing any work.
This convention is referenced by every agent and enforced at the workflow level.

## Time Zones

| Zone | Purpose |
|------|---------|
| **Soft curfew** | 22:00 — warn + redirect to `@scribe` |
| **Hard curfew** | 23:00 — all agents refuse work |

## Rules by Time Window

### Before 22:00 — Normal Operation

All agents operate normally. No restrictions.

### 22:00–23:00 — Shutdown Window

**Check**: Does today's diary exist at `10_Projects/Work_Journal/YYYYMMDD_Work_Diary.md`?

| Diary exists? | Agent is @scribe? | Action |
|---|---|---|
| No | No | **BLOCK**. Print warning, redirect to `@scribe` |
| No | Yes | Proceed with shutdown ritual |
| Yes | No | Allow **minor wrap-up only** (commit, push, save). Remind: "請準備收工" |
| Yes | Yes | Allow supplement or archive tasks only |

**Block message** (for non-scribe agents):

```
⏰ 已過 22:00，今日日記尚未完成。
請先呼叫 @scribe 完成收工儀式，再進行其他工作。
```

### After 23:00 — Hard Curfew

**ALL agents** (including @scribe) refuse to work. Response:

```
⏰ 已超過 23:00。請休息。
明天的工作狀態在各 repo 的 .dev/state.md 中。
```

No exceptions. No "just one more thing."

## Implementation

This convention is enforced by each agent reading `conventions/curfew.md` as part of their startup check. The time check uses the system clock:

```powershell
(Get-Date).Hour
```

## Override

If the user explicitly says "override curfew" or "skip curfew":
- Allow work but print a single reminder: "宵禁已暫時解除。請注意休息。"
- Do NOT nag repeatedly after override is granted
- Override expires at next invocation (does not persist)

## Why This Exists

This is a self-imposed boundary to enforce healthy work habits. The agents serve the user — and part of serving well is protecting the user from overwork.
