---
name: git-commits
description: Write commit messages that follow this project's Conventional Commit format. Use whenever the user asks for a commit message, wants to rewrite one, says to commit or summarize staged changes, or needs wording that matches GAL rules. Analyze staged changes or the provided diff, choose the right commit type, detect breaking-change risk, choose the most relevant scope, and return the repo's canonical scoped format.
---

# Git Commits Skill

## Current State

!`git diff --cached --stat` !`git log --oneline -5` !`git status --short`

## Overview

Analyze staged git changes and generate commit messages that match this repository's canonical Conventional Commit style. Use staged diffs or a provided patch to determine the correct type, detect breaking-change risk, choose the most relevant scope, and produce either a one-line commit or a short body with up to three high-signal bullets.

This repo's local rules override generic Conventional Commits guidance when they conflict. The final output must use a scoped header and stay in this repo's no-footer format unless the user explicitly asks for generic Conventional Commits instead.

## Prerequisites

- Git repository initialized in the working directory
- Changes staged via `git add`, or the user has provided a diff or patch directly
- Understanding that this repo's commit style is canonical, including a scoped header when a primary area can be identified

## Instructions

1. Run git diff --cached --stat to get an overview of staged files and change volume
2. Run git diff --cached to examine the actual code changes in detail
3. Classify the commit type based on the nature of changes:
    - feat: new functionality visible to users
    - fix: bug correction
    - refactor: code restructuring without behavior change
    - docs: documentation only
    - test: adding or updating tests
    - chore: build process, dependencies, or tooling
    - perf: performance improvement
    - ci: CI/CD configuration changes
4. Determine scope from the primary directory or module affected (e.g., auth, api, cli, db)
5. Check for breaking changes: removed public APIs, changed function signatures, renamed exports, schema migrations
6. Check recent commit history with git log --oneline -10 to match the project's style conventions
7. Construct the commit message: type(scope): imperative description under 72 characters
8. For non-trivial changes, add up to three short bullets describing the most important changes and their impact.
9. Do not use footers, if the change is breaking, make that clear in the header or bullets unless the user explicitly asks for generic Conventional Commits.

## Canonical Rules

- Use imperative mood: `add feature`, not `added feature`
- Start with lowercase
- Do not end the subject with a period
- Keep the header under 72 characters when practical
- Describe what changed, not the implementation process
- Use scope in the header: `feat(core):` not `feat:` when a primary area can be identified
- Choose the most relevant scope from the dominant module, directory, command, or feature area
- Do not add footer lines such as `Closes #123`
- Do not use `BREAKING CHANGE:` footers unless the user explicitly asks for generic Conventional Commits instead of this repo's format
- If the change is breaking, make that obvious in the title or bullets without introducing a footer unless the user explicitly asks for one

## Output

Commit message following this repository's format:

```text
<type>(<scope>): <brief description>
```

For broader changes, use this format with at most three bullets:

```text
<type>(<scope>): <brief description>

- most important change
- most important change
- most important change
```

Bullet rules:

- Use zero to three bullets only
- Summarize the most important changes, not file names or file lists
- Keep each bullet short and high signal
- Omit bullets entirely for small, single-purpose changes

## Error Handling

| Error | Cause | Solution |
| --- | --- | --- |
| `No changes staged for commit` | Nothing added to the staging area | Run `git add <files>` to stage changes before generating the message |
| `Not a git repository` | Working directory is not inside a git repo | Run `git init` or navigate to the repository root |
| `Ambiguous commit type` | Changes span multiple categories | Split into separate commits or choose the dominant intent |
| `Scope is unclear` | Changes touch many unrelated areas | Use the most significant module or feature area; omit scope only if no honest scope fits |
| `Commit message is too long` | Description is too verbose | Shorten the header and move only the most important changes into up to three bullets |

## Examples

- "Analyze my staged changes and generate a scoped commit message in this repo's format."
- "Create a commit message for these changes and call out any breaking-change risk without using a footer."
- "Summarize this broader change as a scoped title plus the three most important changes."

## Resources

- [Conventional Commits specification](https://www.conventionalcommits.org/en/v1.0.0/)
- [Git commit best practices](https://cbea.ms/git-commit/)

## Output Requirements

When asked for a commit message:

1. Pick the correct type.
2. Choose the most relevant scope and use a scoped header.
3. Return only the commit message unless the user asks for explanation.
