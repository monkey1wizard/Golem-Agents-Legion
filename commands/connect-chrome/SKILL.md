---
name: connect-chrome
description: "Switch the browser session from headless to headed Chrome with the Side Panel extension loaded. One-time mode switch — call once at session start if you need to observe the browser."
---

# /connect-chrome

Switch to a headed Chrome session with Side Panel enabled.

## Role

Browser mode configurator. Call once, then proceed with `/qa` or `/browse` as normal.

## When to Use

- When you need to observe browser behavior visually during a QA or debug session
- When the app has visual interactions that are easier to verify with a visible browser
- When you want to use the Copilot Side Panel while browsing

## What This Does

1. Closes any existing headless Playwright session
2. Reopens Chrome in headed mode (visible browser window)
3. Loads the Copilot Side Panel extension if installed
4. Shows a green shimmer in the Side Panel header to indicate connected mode

After this command, all `/browse`, `/qa`, and `/design-review` commands use the headed session until the session ends.

## Step 1 — Close Headless Session

If a Playwright session is currently open, close it cleanly.

## Step 2 — Launch Headed Chrome

Launch Chrome with:
- `headless: false`
- `args: ['--disable-web-security', '--auto-open-devtools-for-tabs']`
- Viewport: 1440×900

## Step 3 — Load Side Panel Extension

If the extension path is configured in `.dev/extensions.json`: load it.

If not configured: proceed without it. Tell the user how to configure it if they want it.

## Step 4 — Confirm

Tell the user: "Headed Chrome is connected. All browser commands will now use a visible browser. Green shimmer active."

## No Plan Files

`/connect-chrome` does not write to the plan file.
