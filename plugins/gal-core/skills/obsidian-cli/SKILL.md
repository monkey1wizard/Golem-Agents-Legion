---
name: obsidian-cli
description: Interact with Obsidian vaults using the Obsidian CLI to read, create, search, and manage notes, tasks, properties, and more. Also supports plugin and theme development with commands to reload plugins, run JavaScript, capture errors, take screenshots, and inspect the DOM. Use when the user asks to interact with their Obsidian vault, manage notes, search vault content, perform vault operations from the command line, or develop and debug Obsidian plugins and themes.
cliDependencies:
   required:
      - obsidian
---

# Obsidian CLI

Use the `obsidian` CLI to interact with a running Obsidian instance. **Requires Obsidian to be open.**

## Preferred Tool Order

1. Use the `obsidian` CLI when the task requires vault-aware reads, writes, task updates, or plugin-development operations.
2. Skip only non-critical read-only sanity checks when Obsidian is closed and the result is not required to complete the user request.
3. Block write operations rather than falling back to direct file edits, because this skill depends on Obsidian's own indexing and graph updates.

## Environment

- **Vault name**: `<OBSIDIAN_VAULT_NAME>`
- **Vault path**: `<OBSIDIAN_VAULT>`
- Always prefix commands with `vault="<OBSIDIAN_VAULT_NAME>"` when targeting this vault explicitly.

## Availability Check

Before running any CLI command, check if Obsidian is open:

```bash
obsidian vault="<OBSIDIAN_VAULT_NAME>" tags total
```

- Exit code `0` → Obsidian is open, proceed with CLI.
- Exit code non-zero or error → Obsidian is **closed**. See Fallback Strategy below.

## Fallback Strategy (Obsidian is closed)

When a CLI command is needed but Obsidian is not running, **do NOT silently skip or use file tools as a substitute**. Instead:

1. **Print a clear message** to the user stating exactly which command was intended to run, for example:

   > ⚠️ 我想執行以下 Obsidian CLI 指令，但 Obsidian 目前未開啟：
   > ```
   > obsidian create name="My Note" content="..."
   > ```
   > 請開啟 Obsidian 後告訴我，我會繼續執行。

2. **Pause and wait** for the user to confirm Obsidian is open before retrying.
3. If the operation is purely **read-only and non-critical** (e.g., checking backlinks as a sanity check), the step may be **skipped** with a note that it was omitted. Clearly state what was skipped.
4. **Never silently fall back to file tools** for write operations — the CLI ensures Obsidian's index and graph stay in sync.

## No-Tool Behavior

If Obsidian is closed and the task depends on a write, state the exact command you intended to run and pause for the user to open Obsidian.

- Non-critical read-only checks may be skipped, but the omission must be called out explicitly.
- Write paths, index-sensitive checks, and plugin workflows should stop until the CLI becomes available again.
- Do not claim the vault is updated when the CLI path was unavailable.

## Command reference

Run `obsidian help` to see all available commands. This is always up to date. Full docs: https://obsidian.md/help/cli

## Syntax

**Parameters** take a value with `=`. Quote values with spaces:

```bash
obsidian create name="My Note" content="Hello world"
```

**Flags** are boolean switches with no value:

```bash
obsidian create name="My Note" silent overwrite
```

For multiline content use `\n` for newline and `\t` for tab.

## File targeting

Many commands accept `file` or `path` to target a file. Without either, the active file is used.

- `file=<name>` — resolves like a wikilink (name only, no path or extension needed)
- `path=<path>` — exact path from vault root, e.g. `folder/note.md`

## Vault targeting

Commands target the most recently focused vault by default. Use `vault=<name>` as the first parameter to target a specific vault:

```bash
obsidian vault="My Vault" search query="test"
```

## Common patterns

```bash
obsidian read file="My Note"
obsidian create name="New Note" content="# Hello" template="Template" silent
obsidian append file="My Note" content="New line"
obsidian search query="search term" limit=10
obsidian daily:read
obsidian daily:append content="- [ ] New task"
obsidian property:set name="status" value="done" file="My Note"
obsidian tasks daily todo
obsidian tags sort=count counts
obsidian backlinks file="My Note"
```

Use `--copy` on any command to copy output to clipboard. Use `silent` to prevent files from opening. Use `total` on list commands to get a count.

## Plugin development

### Develop/test cycle

After making code changes to a plugin or theme, follow this workflow:

1. **Reload** the plugin to pick up changes:
   ```bash
   obsidian plugin:reload id=my-plugin
   ```
2. **Check for errors** — if errors appear, fix and repeat from step 1:
   ```bash
   obsidian dev:errors
   ```
3. **Verify visually** with a screenshot or DOM inspection:
   ```bash
   obsidian dev:screenshot path=screenshot.png
   obsidian dev:dom selector=".workspace-leaf" text
   ```
4. **Check console output** for warnings or unexpected logs:
   ```bash
   obsidian dev:console level=error
   ```

### Additional developer commands

Run JavaScript in the app context:

```bash
obsidian eval code="app.vault.getFiles().length"
```

Inspect CSS values:

```bash
obsidian dev:css selector=".workspace-leaf" prop=background-color
```

Toggle mobile emulation:

```bash
obsidian dev:mobile on
```

Run `obsidian help` to see additional developer commands including CDP and debugger controls.
