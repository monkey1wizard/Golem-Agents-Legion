---
name: git-commit-msg
description: "Generate a Conventional Commit message from the current staged changes using compact staged git context."
---

# /git-commit-msg

Generate a dynamic Conventional Commit message for the current staged diff.

## Goal

Run `gal commit-msg --print` for the deterministic `type(scope)` baseline, then author a subject that describes what actually changed.

## Baseline Output

!`gal commit-msg --print`

## Instructions

- Use the baseline above for the `type(scope):` prefix (it is a no-hijack path/status classifier — it never reads the diff body).
- Read the staged diff and write a subject that says what actually changed; do not ship the baseline's generic subject (e.g. `refactor crates`) verbatim.
- Keep the baseline `type(scope)` prefix unless the diff clearly contradicts it.
- Add a body of up to three bullets only for broader changes, summarizing the most important changes — not file names.
- Do not add explanations, markdown fences, reasoning tags, or extra prose.
- If the baseline reports `No changes staged for commit.` or `Not a git repository.`, return that text exactly.

## Output

Return only the final commit message in this repo's `type(scope): subject` format.
