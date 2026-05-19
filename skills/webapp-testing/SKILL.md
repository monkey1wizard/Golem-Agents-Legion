---
name: webapp-testing
description: Toolkit for interacting with and testing local web applications using Playwright. Supports verifying frontend functionality, debugging UI behavior, capturing browser screenshots, and viewing browser logs.
mcpDependencies:
  optional:
    - playwright
    - chrome-devtools
license: Complete terms in LICENSE.txt
---

# Web Application Testing

## Preferred Tool Order

1. Route by task shape instead of forcing a single browser tool.
2. Use Playwright MCP first for live interaction, forms, uploads, session setup, responsive checks, and browser-backed assertions.
3. Use Chrome DevTools MCP for console, network, protocol, DOM, performance, and accessibility diagnostics.
4. Switch to native Playwright scripts when you need reusable automation or flows the MCP routes cannot express cleanly.
5. Treat this as an explicit browser-visible exception to generic CLI-first patterns.

## Route Selection

- Use `playwright` MCP for interactive browser work: navigation, clicks, fills, uploads, auth/session preparation, screenshots, and task-scoped assertions against the rendered UI.
- Use `chrome-devtools` MCP for inspection-heavy work: console output, network behavior, DOM snapshots, performance clues, protocol-level diagnostics, and accessibility tree checks.
- Use native Playwright scripts when you need reusable automation, multi-page orchestration, helper-script integration, CI-friendly scripts, or behavior the MCP routes cannot express cleanly.

If more than one route is viable, prefer the route that yields the smallest honest reproduction with the clearest evidence.

## Required Result Labeling

Always report which route actually ran:

- `Browser Route: Playwright MCP`
- `Browser Route: Chrome DevTools MCP`
- `Browser Route: Native Playwright`
- `Browser Route: No runnable browser route`

If no runnable route exists, stop and mark the scenario `BLOCKED` instead of inferring success from static code or screenshots.

## No-Tool Behavior

If neither Playwright MCP, Chrome DevTools MCP, nor runnable Playwright automation are available, stop and report that browser automation capability is missing.

- Do not fake UI verification from static assumptions.
- Do not claim an interaction path was tested if no runnable browser path existed.

To test local web applications, write native Python Playwright scripts.

**Helper Scripts Available**:
- `scripts/with_server.py` - Manages server lifecycle (supports multiple servers)

**Always run scripts with `--help` first** to see usage. DO NOT read the source until you try running the script first and find that a customized solution is abslutely necessary. These scripts can be very large and thus pollute your context window. They exist to be called directly as black-box scripts rather than ingested into your context window.

## Decision Tree: Choosing Your Approach

```
User task → Is it static HTML?
    ├─ Yes → Read HTML file directly to identify selectors
    │         ├─ Success → Write Playwright script using selectors
    │         └─ Fails/Incomplete → Treat as dynamic (below)
    │
    └─ No (dynamic webapp) → Is the app already running?
      ├─ No → Start it from the repo's dev instructions first
      │      Then choose the route below
      └─ Yes → What evidence is required?
        ├─ Live interaction, forms, files, session state, responsive UI
        │   └─ Use Playwright MCP when ready
        ├─ Console, network, protocol, perf, accessibility, DOM diagnostics
        │   └─ Use Chrome DevTools MCP when ready
        ├─ Reusable automation or MCP route unavailable/not expressive enough
        │   └─ Run: python scripts/with_server.py --help
        │      Then use the helper + write simplified Playwright script
        └─ No runnable route available
          └─ Report BLOCKED; do not claim browser validation ran
```

## Example: Using with_server.py

To start a server, run `--help` first, then use the helper:

**Single server:**
```bash
python scripts/with_server.py --server "npm run dev" --port 5173 -- python your_automation.py
```

**Multiple servers (e.g., backend + frontend):**
```bash
python scripts/with_server.py \
  --server "cd backend && python server.py" --port 3000 \
  --server "cd frontend && npm run dev" --port 5173 \
  -- python your_automation.py
```

To create an automation script, include only Playwright logic (servers are managed automatically):
```python
from playwright.sync_api import sync_playwright

with sync_playwright() as p:
    browser = p.chromium.launch(headless=True) # Always launch chromium in headless mode
    page = browser.new_page()
    page.goto('http://localhost:5173') # Server already running and ready
    page.wait_for_load_state('networkidle') # CRITICAL: Wait for JS to execute
    # ... your automation logic
    browser.close()
```

## Reconnaissance-Then-Action Pattern

1. **Inspect rendered DOM**:
   ```python
   page.screenshot(path='/tmp/inspect.png', full_page=True)
   content = page.content()
   page.locator('button').all()
   ```

2. **Identify selectors** from inspection results

3. **Execute actions** using discovered selectors

## Common Pitfall

❌ **Don't** inspect the DOM before waiting for `networkidle` on dynamic apps
✅ **Do** wait for `page.wait_for_load_state('networkidle')` before inspection

## Best Practices

- **Use bundled scripts as black boxes** - To accomplish a task, consider whether one of the scripts available in `scripts/` can help. These scripts handle common, complex workflows reliably without cluttering the context window. Use `--help` to see usage, then invoke directly. 
- Route by evidence needs, not habit. Playwright MCP is the default interactive path; Chrome DevTools MCP is the default diagnostics path.
- Use `sync_playwright()` for synchronous scripts
- Always close the browser when done
- Use descriptive selectors: `text=`, `role=`, CSS selectors, or IDs
- Add appropriate waits: `page.wait_for_selector()` or `page.wait_for_timeout()`

## Reference Files

- **examples/** - Examples showing common patterns:
  - `element_discovery.py` - Discovering buttons, links, and inputs on a page
  - `static_html_automation.py` - Using file:// URLs for local HTML
  - `console_logging.py` - Capturing console logs during automation