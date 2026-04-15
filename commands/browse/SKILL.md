---
name: browse
description: "Playwright/Chromium capability primitive. Provides goto, snapshot, fill, click, screenshot, console log, and session handoff. Used internally by /qa, /qa-only, /design-review, and /land-and-deploy — not a standalone workflow command."
---

# /browse

Playwright browser control primitive. Used by other commands — not a standalone workflow.

## Role

Reliable browser driver. Get the page into the right state for the calling command.

## When to Use

Called internally by:
- `/qa` — real browser testing
- `/qa-only` — report-only browser testing
- `/design-review` — live-site screenshots
- `/land-and-deploy` — production verification after deploy

Do not use directly unless you need raw browser access outside the above workflows.

## Session Lifecycle

A `/browse` session stays open across multiple commands in the same conversation. Idle shutdown after 30 minutes of no commands.

If the session was closed, reopen it automatically — do not ask the user.

## Auto-Handoff Rule

After 3 consecutive failures on a single page action:
1. Take a screenshot of the current state
2. Report what was attempted and what failed
3. Ask the user: "I'm stuck on this step. Would you like to take over in the browser, then tell me when to resume?"
4. Resume automatically when the user confirms

## Commands

### goto
Navigate to a URL.
```
goto <url>
```
Wait for `networkidle` before returning. Take a screenshot on completion.

### snapshot
Return the full page DOM as text. Use to locate element selectors before fill/click.

### fill
Fill a form field.
```
fill <selector> <value>
```

### click
Click an element.
```
click <selector>
```
Wait for any triggered navigation or network activity before returning.

### screenshot
Take a viewport screenshot and save to `docs/screenshots/<slug>-<timestamp>.png`.

### console
Return all browser console messages since session start (info, warning, error).

### handoff
Pause browser control and open a description of the current URL + page state. Used for auto-handoff.

### resume
Resume browser control after a user handoff.

## Failure Handling

On any Playwright error:
1. Log the error type and message
2. Take a screenshot
3. Increment the failure counter for this page
4. If failure counter ≥ 3: trigger auto-handoff

## No Plan Artifacts

`/browse` does not write to the plan file. The calling command is responsible for recording what was found.
