---
name: git-commit-msg
description: "Generate a Conventional Commit message from the current staged changes using compact staged git context."
---

# /git-commit-msg

Generate a dynamic Conventional Commit message for the current staged diff.

## Goal

Run the repo helper that classifies the current staged diff and return its output exactly.

## Windows Helper Output

!`pwsh -NoProfile -File ./scripts/Get-StagedCommitMessage.ps1`

## Non-Windows Helper Output

Run `./scripts/get-staged-commit-message.sh` when the PowerShell helper is unavailable.

## Instructions

- Return the helper output exactly.
- The helper decides whether the result is header-only or includes a body.
- Do not invent bullets or extend the body beyond helper output.
- Do not add explanations, markdown fences, reasoning tags, or extra prose.
- If the helper reports `No changes staged for commit.` or `Not a git repository.`, return that text exactly.

## Output

The helper already returns the final commit message in this repo's format.

Return only that final text.