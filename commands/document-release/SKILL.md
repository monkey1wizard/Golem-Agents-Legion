---
name: document-release
description: "Technical Writer — cross-references the git diff against all doc files, updates file paths, command lists, structure trees, feature tables, and cleans up completed TODOs. Auto-invoked by /ship. Also updates commands/commands.md when commands change."
---

# /document-release

Keep all documentation current after code changes.

## Role

Technical writer. Every doc file should reflect the current state of the code.

## When to Use

- Auto-invoked by `/ship` after PR creation
- Any time you want to sync docs manually after a batch of commits
- When you notice doc files are out of sync with the codebase

## Step 1 — Read All Doc Files

Collect every documentation file in the project:
- `README.md`
- `ARCHITECTURE.md` (if exists)
- `CONTRIBUTING.md` (if exists)
- `CLAUDE.md` (if exists)
- `TESTING.md` (if exists)
- `CHANGELOG.md` (if exists)
- `TODOS.md` or `TODO.md` (if exists)
- `docs/**/*.md`
- `commands/commands.md` (GAL-specific)

## Step 2 — Read the Diff

Read `git diff main...HEAD` to see what changed.

## Step 3 — Cross-Reference and Update

For each doc file, check against the diff:

### File paths and imports
- Any path reference that no longer exists → update or remove
- Any new file that should be mentioned → add

### Command lists and tables
- Any command added or removed → update tables
- For `commands/commands.md`: add any new commands created in this branch
- For GAL `README.md`: update command catalog table

### Project structure trees
- ASCII directory trees that include changed paths → regenerate

### Feature tables
- Capability matrices, feature comparison tables → update rows

### CHANGELOG
- Do not overwrite existing entries
- If the branch adds a significant feature: add a new entry at the top
- Polish voice (past tense, imperative verbs) without changing meaning
- Only bump `VERSION` if the user asks or if a major feature was added

### TODOS / task lists
- Any TODO marked with a commit hash or task ID that matches a commit in this branch → mark as completed or remove
- Do not remove TODOs that are not clearly addressed by this branch

### Cross-doc consistency
- Ensure the same version number appears consistently across all files
- Ensure the same feature names are used consistently

## Step 4 — Surfaces Judgment Calls Only

For any change that requires taste or strategy decisions (e.g. "should we rename a section?", "is this feature worth a major version bump?"):
- Ask the user via `AskUserQuestion`
- Do not make subjective editorial changes silently

## Step 5 — Commit Updates

Commit all doc updates in a single commit:
`docs: update documentation for <branch-name>`

## Step 6 — Update PR Body

If a PR is open for this branch: update the PR body to include a `## Docs updated` section listing which files were changed.

Tell the user: which doc files were updated and what changed in each.
