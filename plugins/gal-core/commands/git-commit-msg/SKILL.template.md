---
name: git-commit-msg
description: "Generate a Conventional Commit message from the current staged changes using compact staged git context."
---

# /git-commit-msg

Generate a dynamic Conventional Commit message for the current staged diff.

## Goal

Run `gal commit-msg --context` once for the FILES/BASELINE/PLANS/PROMPTS/CHANGES blocks, then author a subject that describes what actually changed.

## Baseline Output

!`gal commit-msg --context`

## Instructions

- Use `BASELINE` above for the `type(scope):` prefix (it is a no-hijack path/status classifier — it never reads the diff body).
- Write a subject that says what actually changed, using `PLANS`'s Goal for staged plan files and `CHANGES`'s hunk headers for code files — do not read the raw diff yourself. Do not ship `BASELINE`'s generic subject (e.g. `refactor crates`) verbatim.
- Keep the `BASELINE` `type(scope)` prefix unless the context clearly contradicts it.
- Add a body of up to three bullets only for broader changes, summarizing the most important changes — not file names.
- Do not add explanations, markdown fences, reasoning tags, or extra prose.
- If the output reports `No staged changes.`, return that text exactly.

## Output

Return only the final commit message in this repo's `type(scope): subject` format.
